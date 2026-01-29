use crate::vm::Vm;
use kvm_ioctls::VcpuExit;
use kvm_bindings::Msrs;
use kvm_bindings::kvm_msr_entry;
use std::io::{self, Write};
use crate::debug;

pub fn run(vm: &mut Vm) -> Result<(), Box<dyn std::error::Error>> {
    setup_long_mode(vm)?;
    
    let kpcr_vaddr: u64 = 0xFFFFF80006000000;
    let msrs = Msrs::from_entries(&[
        kvm_msr_entry { index: 0xc0000101, data: kpcr_vaddr, ..Default::default() }, 
        kvm_msr_entry { index: 0xc0000102, data: kpcr_vaddr, ..Default::default() }, 
    ]).unwrap();
    vm.vcpu_fd.set_msrs(&msrs).ok();

    println!("\n--- KVM vCPU Start ---");

    loop {
        // vCPU 실행 결과를 별도의 스코프에서 처리하여 vm 대여 수명을 관리합니다.
        let action = {
            let exit_reason = vm.vcpu_fd.run()?;
            match exit_reason {
                VcpuExit::IoOut(addr, data) => {
                    if (addr == 0x3F8 || addr == 0xF8) && !data.is_empty() {
                        print!("{}", data[0] as char);
                        io::stdout().flush()?;
                        None 
                    } else if addr == 0xF9 && !data.is_empty() {
                        Some(LoopAction::Trap(data[0]))
                    } else {
                        None
                    }
                }
                VcpuExit::Hlt => Some(LoopAction::Dump("HLT".to_string())),
                VcpuExit::Shutdown => Some(LoopAction::Dump("Shutdown".to_string())),
                VcpuExit::MmioWrite(addr, _data) => {
                    if addr > 0xFFFFFFFF || addr < 0x1000 { None } 
                    else { Some(LoopAction::Dump(format!("MMIO Write 0x{:x}", addr))) }
                }
                VcpuExit::MmioRead(addr, _data) => {
                    if addr > 0xFFFFFFFF || addr < 0x1000 { None }
                    else { Some(LoopAction::Dump(format!("MMIO Read 0x{:x}", addr))) }
                }
                other => Some(LoopAction::Dump(format!("{:?}", other))),
            }
        };

        // 스코프를 벗어나 vm 대여가 해제된 후 디버그 함수를 호출합니다.
        match action {
            Some(LoopAction::Trap(v)) => return debug::handle_diagnostic_trap(vm, v),
            Some(LoopAction::Dump(msg)) => {
                println!("\nKVM: {}", msg);
                return debug::dump_all_registers(vm);
            }
            None => continue,
        }
    }
}

enum LoopAction {
    Trap(u8),
    Dump(String),
}

fn setup_long_mode(vm: &mut Vm) -> Result<(), Box<dyn std::error::Error>> {
    let gdt_base: u64 = crate::SYSTEM_BASE;
    let tss_base: u64 = gdt_base + 0x200;
    let kpcr_vaddr: u64 = 0xFFFFF80006000000; 
    let kernel_stack_v: u64 = 0xFFFFF80000090000; 
    
    let cpuid = vm.kvm.get_supported_cpuid(kvm_bindings::KVM_MAX_CPUID_ENTRIES)?;
    vm.vcpu_fd.set_cpuid2(&cpuid)?;

    let mut gdt: [u64; 32] = [0; 32];
    gdt[1] = 0x00cf9a000000ffff; gdt[2] = 0x00af9a000000ffff; gdt[3] = 0x00cf92000000ffff; 
    gdt[4] = 0x00cffb000000ffff; gdt[5] = 0x00cff3000000ffff; gdt[6] = 0x00affb000000ffff; 
    gdt[7] = 0x00cff3000000ffff; 
    let tss_limit = 104 - 1;
    gdt[8] = (tss_base & 0xffffff) << 16 | (tss_base & 0xff000000) << 32 | 0x0000890000000000 | tss_limit;
    gdt[9] = tss_base >> 32;
    gdt[10] = 0x00cff2000000ffff; gdt[11] = 0x00cff2000000ffff;

    let mut gdt_bytes = Vec::new();
    for entry in &gdt { gdt_bytes.extend_from_slice(&entry.to_le_bytes()); }
    vm.write_memory(gdt_base as usize, &gdt_bytes)?;

    let mut tss = [0u8; 104];
    tss[4..12].copy_from_slice(&kernel_stack_v.to_le_bytes());
    vm.write_memory(tss_base as usize, &tss)?;

    vm.vcpu_fd.set_fpu(&kvm_bindings::kvm_fpu::default())?;
    let mut sregs = vm.vcpu_fd.get_sregs()?;
    sregs.cr3 = gdt_base + 0x1000;
    sregs.cr4 = (1 << 5) | (1 << 9) | (1 << 10) | (1 << 16) | (1 << 18); 
    sregs.efer = (1 << 0) | (1 << 8) | (1 << 10) | (1 << 11); 
    sregs.cr0 = (1 << 31) | (1 << 0) | (1 << 1) | (1 << 16); 
    sregs.gdt.base = gdt_base;
    sregs.gdt.limit = (32 * 8 - 1) as u16;
    sregs.idt.base = 0;
    sregs.idt.limit = 0xFFF;

    fn seg_64(selector: u16, is_code: bool) -> kvm_bindings::kvm_segment {
        kvm_bindings::kvm_segment {
            base: 0, limit: 0xffffffff, selector, present: 1,
            type_: if is_code { 11 } else { 3 },
            s: 1, l: if is_code { 1 } else { 0 }, g: 1, db: 0,
            ..kvm_bindings::kvm_segment::default()
        }
    }
    sregs.cs = seg_64(0x10, true);
    let ds = seg_64(0x18, false);
    sregs.ds = ds; sregs.es = ds; sregs.ss = ds;
    sregs.gs = ds;
    sregs.gs.base = kpcr_vaddr; 

    sregs.tr = kvm_bindings::kvm_segment { 
        base: tss_base, limit: tss_limit as u32, selector: 0x40, 
        type_: 9, present: 1, s: 0, g: 0, ..kvm_bindings::kvm_segment::default() 
    };
    vm.vcpu_fd.set_sregs(&sregs)?;

    let msr_entries = [
        kvm_msr_entry { index: 0xc0000080, data: sregs.efer, ..Default::default() }, 
        kvm_msr_entry { index: 0xc0000081, data: 0x001B001000000000, ..Default::default() }, 
        kvm_msr_entry { index: 0xc0000101, data: kpcr_vaddr, ..Default::default() }, 
        kvm_msr_entry { index: 0xc0000102, data: kpcr_vaddr, ..Default::default() }, 
        kvm_msr_entry { index: 0x277, data: 0x0007040600070406, ..Default::default() }, 
        kvm_msr_entry { index: 0x1b, data: 0xfee00000 | 0x900, ..Default::default() }, 
    ];
    let msrs = Msrs::from_entries(&msr_entries).unwrap();
    vm.vcpu_fd.set_msrs(&msrs).ok();

    let mut regs = vm.vcpu_fd.get_regs()?;
    regs.rflags = 0x2;
    vm.vcpu_fd.set_regs(&regs)?;
    Ok(())
}
