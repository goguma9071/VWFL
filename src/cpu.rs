use crate::vm::Vm;
use kvm_ioctls::VcpuExit;
use kvm_bindings::{kvm_msr_entry, Msrs};
use std::io::{self, Write};
use crate::debug;

pub fn run(vm: &mut Vm, krnl_entry_v: u64, stack_v: u64, lpb_v: u64) -> Result<(), Box<dyn std::error::Error>> {
    println!("[CPU] Initializing vCPU state...");
    setup_long_mode(vm, krnl_entry_v, stack_v, lpb_v)?;
    
    println!("\n--- KVM vCPU Start ---");

    loop {
        // RBP 정화 (비정규 주소 방어막 유지)
        {
            let mut regs = vm.vcpu_fd.get_regs()?;
            let rbp_prefix = regs.rbp >> 47;
            if rbp_prefix != 0 && rbp_prefix != 0x1FFFF {
                regs.rbp = 0;
                vm.vcpu_fd.set_regs(&regs).ok();
            }
        }

        let exit_reason = vm.vcpu_fd.run()?;

        // [FIX] MMIO 및 I/O 핸들링 로직
        let action = match exit_reason {
            VcpuExit::IoOut(addr, data) => {
                if (addr == 0x3F8 || addr == 0xF8 || addr == 0x80) && !data.is_empty() {
                    print!("{}", data[0] as char);
                    io::stdout().flush()?;
                    None 
                } else if addr == 0xF9 && !data.is_empty() {
                    Some(LoopAction::Trap(data[0]))
                } else {
                    None
                }
            }
            VcpuExit::MmioWrite(addr, data) => {
                // 커널이 하드웨어 레지스터에 쓰는 것을 로그로 남기고 계속 진행
                println!("[MMIO] Write: 0x{:08x} = {:?}", addr, data);
                None
            }
            VcpuExit::MmioRead(addr, _data) => {
                // 읽기 시도 시 기본적으로 0을 반환 (필요 시 나중에 장치 에뮬레이션 추가)
                println!("[MMIO] Read : 0x{:08x}", addr);
                None
            }
            VcpuExit::Hlt => Some(LoopAction::Dump("HLT".to_string())),
            VcpuExit::Shutdown => Some(LoopAction::Dump("Shutdown (Triple Fault)".to_string())),
            other => Some(LoopAction::Dump(format!("{:?}", other))),
        };

        sync_kernel_state(vm)?;

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

fn sync_kernel_state(vm: &mut Vm) -> Result<(), Box<dyn std::error::Error>> {
    let mut msrs = Msrs::from_entries(&[
        kvm_msr_entry { index: 0xc0000101, ..Default::default() }, 
        kvm_msr_entry { index: 0xc0000102, ..Default::default() }, 
    ]).unwrap();
    
    if vm.vcpu_fd.get_msrs(&mut msrs).is_ok() {
        let entries = msrs.as_slice();
        let gs = entries[0].data;
        let kgs = entries[1].data;
        if gs != kgs && gs >= 0xFFFFF80000000000 {
            let new_msrs = Msrs::from_entries(&[
                kvm_msr_entry { index: 0xc0000102, data: gs, ..Default::default() },
            ]).unwrap();
            vm.vcpu_fd.set_msrs(&new_msrs).ok();
        }
    }

    let regs = vm.vcpu_fd.get_regs()?;
    if regs.rsp >= 0xFFFFF80000000000 {
        let tss_pbase = crate::SYSTEM_BASE + 0x1000;
        vm.write_memory(tss_pbase as usize + 4, &regs.rsp.to_le_bytes()).ok();
    }
    
    Ok(())
}

enum LoopAction {
    Trap(u8),
    Dump(String),
}

fn setup_long_mode(vm: &mut Vm, krnl_entry_v: u64, stack_v: u64, lpb_v: u64) -> Result<(), Box<dyn std::error::Error>> {
    let k_virt_base: u64 = 0xFFFFF80000000000;
    let gdt_pbase: u64 = crate::SYSTEM_BASE;
    let tss_pbase: u64 = gdt_pbase + 0x1000;
    let gdt_vbase = k_virt_base + gdt_pbase;
    let tss_vbase = k_virt_base + tss_pbase;
    let kpcr_vaddr: u64 = lpb_v + 0xae80; 
    
    let mut cpuid = vm.kvm.get_supported_cpuid(kvm_bindings::KVM_MAX_CPUID_ENTRIES)?;
    for entry in cpuid.as_mut_slice() {
        if entry.function == 0x80000001 { entry.edx |= 1 << 20; }
    }
    vm.vcpu_fd.set_cpuid2(&cpuid)?;

    let mut gdt: [u64; 32] = [0; 32];
    gdt[1] = 0x00af9a000000ffff; 
    gdt[2] = 0x00af9a000000ffff; 
    gdt[3] = 0x00cf92000000ffff; 
    gdt[4] = 0x00affb000000ffff; 
    gdt[5] = 0x00cff3000000ffff; 
    gdt[6] = 0x00affb000000ffff; 
    gdt[7] = 0x00cff3000000ffff;

    let tss_limit = 104 - 1;
    let tss_low = (tss_vbase & 0xffffff) << 16 | (tss_vbase & 0xff000000) << 32 | 0x0000890000000000 | tss_limit;
    let tss_high = tss_vbase >> 32;
    gdt[8] = tss_low;
    gdt[9] = tss_high;
    gdt[10] = 0x00cff3000000ffff; 

    let mut gdt_bytes = Vec::new();
    for entry in &gdt { gdt_bytes.extend_from_slice(&entry.to_le_bytes()); }
    vm.write_memory(gdt_pbase as usize, &gdt_bytes)?;

    let mut tss = [0u8; 104];
    tss[4..12].copy_from_slice(&stack_v.to_le_bytes()); 
    for i in 0..7 {
        let offset = 36 + (i * 8);
        tss[offset..offset+8].copy_from_slice(&stack_v.to_le_bytes());
    }
    vm.write_memory(tss_pbase as usize, &tss)?;

    vm.vcpu_fd.set_fpu(&kvm_bindings::kvm_fpu::default())?;
    let mut sregs = vm.vcpu_fd.get_sregs()?;
    sregs.cr3 = gdt_pbase + 0x2000;
    sregs.cr4 = (1 << 5) | (1 << 9) | (1 << 10) | (1 << 16); 
    sregs.efer = (1 << 0) | (1 << 8) | (1 << 10) | (1 << 11); 
    sregs.cr0 = (1 << 31) | (1 << 0) | (1 << 1) | (1 << 16); 
    
    sregs.gdt.base = gdt_vbase;
    sregs.gdt.limit = (32 * 8 - 1) as u16;
    sregs.idt.base = k_virt_base; 
    sregs.idt.limit = 0x0FFF;

    fn seg_64(selector: u16, is_code: bool, dpl: u8) -> kvm_bindings::kvm_segment {
        kvm_bindings::kvm_segment {
            base: 0, limit: 0xffffffff, selector, present: 1,
            type_: if is_code { 11 } else { 3 },
            s: 1, l: if is_code { 1 } else { 0 }, g: 1, db: 0, dpl,
            ..kvm_bindings::kvm_segment::default()
        }
    }
    sregs.cs = seg_64(0x10, true, 0);
    let ds = seg_64(0x18, false, 0);
    sregs.ds = ds; sregs.es = ds; sregs.ss = ds;
    sregs.gs = ds;
    sregs.gs.base = kpcr_vaddr;

    sregs.tr = kvm_bindings::kvm_segment { 
        base: tss_vbase, limit: tss_limit as u32, selector: 0x40, 
        type_: 9, present: 1, s: 0, g: 0, dpl: 0, ..kvm_bindings::kvm_segment::default() 
    };
    vm.vcpu_fd.set_sregs(&sregs)?;

    let msr_entries = [
        kvm_msr_entry { index: 0xc0000080, data: sregs.efer, ..Default::default() }, 
        kvm_msr_entry { index: 0xc0000081, data: 0x0023001000000000, ..Default::default() }, 
        kvm_msr_entry { index: 0xc0000082, data: krnl_entry_v, ..Default::default() }, 
        kvm_msr_entry { index: 0xc0000084, data: 0x4700, ..Default::default() }, 
        kvm_msr_entry { index: 0xc0000101, data: kpcr_vaddr, ..Default::default() }, 
        kvm_msr_entry { index: 0xc0000102, data: kpcr_vaddr, ..Default::default() }, 
        kvm_msr_entry { index: 0x1b, data: 0xfee00000 | 0x900, ..Default::default() }, 
    ];
    let msrs = Msrs::from_entries(&msr_entries).unwrap();
    vm.vcpu_fd.set_msrs(&msrs).ok();

    let mut regs = vm.vcpu_fd.get_regs()?;
    regs.rax = 0; regs.rbx = 0; regs.rcx = lpb_v; regs.rdx = lpb_v; 
    regs.r8 = 0; regs.r9 = lpb_v; 
    regs.rbp = 0; 
    regs.rip = krnl_entry_v;
    regs.rsp = stack_v - 0x100;
    regs.rflags = 0x2;
    vm.vcpu_fd.set_regs(&regs)?;

    Ok(())
}