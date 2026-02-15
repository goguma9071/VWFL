use crate::vm::Vm;
use kvm_ioctls::VcpuExit;
use kvm_bindings::{kvm_msr_entry, Msrs};
use crate::debug::dump_all_registers;
use std::io::{self, Write};
use crate::debug;

pub fn run(vm: &mut Vm, krnl_entry_v: u64, stack_v: u64, lpb_v: u64) -> Result<(), Box<dyn std::error::Error>> {
    println!("[CPU] Initializing vCPU state...");
    setup_long_mode(vm, krnl_entry_v, stack_v, lpb_v)?;
    dump_all_registers(vm)?;
    
    // [FORCE DEBUG] Enable Single-Step trace
    vm.vcpu_fd.set_guest_debug(&kvm_bindings::kvm_guest_debug {
        control: 0x00000001 | 0x00000002, 
        ..Default::default() 
    }).ok();

    println!("\n--- KVM vCPU Start (Instruction Trace Mode) ---");

    let mut loop_count: u64 = 0;

    loop {
        loop_count += 1;
        
        let exit_reason = vm.vcpu_fd.run()?; 

        // 1. 필요한 정보를 미리 복사하여 대여 충돌 방지
        let action = match exit_reason {
            VcpuExit::Debug(_) => LoopAction::Trace,
            VcpuExit::IoOut(addr, data) => {
                let val = if data.is_empty() { 0 } else { data[0] };
                if addr == 0xF9 { LoopAction::Trap(val) }
                else if addr == 0x3F8 || addr == 0xF8 || addr == 0x80 { LoopAction::SerialOut(val) }
                else { LoopAction::LogIoOut(addr, val) }
            }
            VcpuExit::IoIn(addr, _) => LoopAction::LogIoIn(addr),
            VcpuExit::Hlt => LoopAction::Dump("HLT".to_string()),
            VcpuExit::Shutdown => LoopAction::Dump("Shutdown (Triple Fault)".to_string()),
            _ => LoopAction::None,
        };

        // 2. 이제 vcpu_fd 대여가 해제되었으므로 안전하게 레지스터 읽기 가능
        match action {
            LoopAction::Trace => {
                if loop_count % 100 == 0 {
                    let regs = vm.vcpu_fd.get_regs().ok();
                    println!("[TRACE] RIP: 0x{:016x?} | Count: {}", regs.map(|r| r.rip), loop_count);
                }
            }
            LoopAction::SerialOut(c) => {
                print!("{}", c as char);
                io::stdout().flush()?;
            }
            LoopAction::LogIoOut(addr, val) => {
                let regs = vm.vcpu_fd.get_regs().ok();
                println!("[IO OUT] Port: 0x{:x}, Data: 0x{:x} | RIP: 0x{:x?}", addr, val, regs.map(|r| r.rip));
            }
            LoopAction::LogIoIn(addr) => {
                let regs = vm.vcpu_fd.get_regs().ok();
                println!("[IO IN ] Port: 0x{:x} | RIP: 0x{:x?}", addr, regs.map(|r| r.rip));
            }
            LoopAction::Trap(v) => {
                debug::handle_diagnostic_trap(vm, v)?;
            }
            LoopAction::Dump(msg) => {
                println!("\nKVM EXIT: {}", msg);
                return debug::dump_all_registers(vm);
            }
            LoopAction::HandleDebug => {
                debug::handle_guest_debug(vm)?;
            }
            LoopAction::None => {}
        }

        if loop_count % 10000 == 0 {
            let regs = vm.vcpu_fd.get_regs().ok();
            println!("[DEBUG] Alive | Loop: {} | RIP: 0x{:016x?}", loop_count, regs.map(|r| r.rip));
        }
    }
}

enum LoopAction {
    None,
    Trace,
    SerialOut(u8),
    LogIoOut(u16, u8),
    LogIoIn(u16),
    Trap(u8),
    HandleDebug,
    Dump(String),
}

fn setup_long_mode(vm: &mut Vm, krnl_entry_v: u64, stack_v: u64, lpb_v: u64) -> Result<(), Box<dyn std::error::Error>> {
    let k_virt_base: u64 = 0xFFFFF80000000000;
    let gdt_pbase: u64 = crate::SYSTEM_BASE;
    let tss_pbase: u64 = gdt_pbase + 0x1000;
    let gdt_vbase = k_virt_base + gdt_pbase;
    let tss_vbase = k_virt_base + tss_pbase;
    let kpcr_vaddr: u64 = lpb_v + 0x10000; 
    let bridge_vaddr: u64 = 0xFFFFF80000000000 + crate::SYSTEM_BASE + 0x100000 + 0x50000; 
    
    let mut cpuid = vm.kvm.get_supported_cpuid(kvm_bindings::KVM_MAX_CPUID_ENTRIES)?;
    for entry in cpuid.as_mut_slice() {
        if entry.function == 0x1 {
            entry.ecx &= !(1 << 31); 
        }
        if entry.function == 0x40000000 {
            entry.ebx = 0; entry.ecx = 0; entry.edx = 0;
        }
        if entry.function == 0x80000001 { entry.edx |= 1 << 20; }
    }
    vm.vcpu_fd.set_cpuid2(&cpuid)?;

    let mut gdt: [u64; 32] = [0; 32];
    gdt[1] = 0x00af9a000000ffff; 
    gdt[2] = 0x00af9a000000ffff; 
    gdt[3] = 0x00cf92000000ffff; 
    gdt[4] = 0x00affb000000ffff; 
    gdt[5] = 0x00cff3000000ffff; 
    gdt[10] = 0x00cff3000000ffff; 

    let tss_limit = 104 - 1;
    let tss_low = (tss_vbase & 0xffffff) << 16 | (tss_vbase & 0xff000000) << 32 | 0x0000890000000000 | tss_limit;
    let tss_high = tss_vbase >> 32;
    gdt[8] = tss_low;
    gdt[9] = tss_high;

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
    sregs.cr3 = gdt_pbase + 0x100000 + 0x2000;
    sregs.cr4 = (1 << 5) | (1 << 9) | (1 << 10) | (1 << 16); 
    sregs.efer = (1 << 0) | (1 << 8) | (1 << 10) | (1 << 11); 
    sregs.cr0 = (1 << 31) | (1 << 0) | (1 << 1) | (1 << 16) | (1 << 21) | (1 << 18);
    
    sregs.gdt.base = gdt_vbase;
    sregs.gdt.limit = (32 * 8 - 1) as u16;
    sregs.idt.base = k_virt_base + gdt_pbase + 0x20000; 
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
        kvm_msr_entry { index: 0xc0000082, data: bridge_vaddr, ..Default::default() }, 
        kvm_msr_entry { index: 0xc0000084, data: 0x4700, ..Default::default() }, 
        kvm_msr_entry { index: 0xc0000101, data: kpcr_vaddr, ..Default::default() }, 
        kvm_msr_entry { index: 0xc0000102, data: kpcr_vaddr, ..Default::default() }, 
        kvm_msr_entry { index: 0x1b, data: 0xfee00000 | 0x900, ..Default::default() }, 
    ];
    let msrs = Msrs::from_entries(&msr_entries).unwrap();
    vm.vcpu_fd.set_msrs(&msrs).expect("Failed to set MSRs");

    let mut regs = vm.vcpu_fd.get_regs()?;
    regs.rip = krnl_entry_v;
    regs.rsp = stack_v - 0x100;
    regs.rflags = 0x2;
    regs.rcx = lpb_v; 
    regs.rdx = lpb_v; 

    vm.vcpu_fd.set_regs(&regs)?;
    Ok(())
}
