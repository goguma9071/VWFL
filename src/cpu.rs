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
    
    println!("\n--- KVM vCPU Start ---");

    let mut loop_count: u64 = 0;
    let mut serial_detected = false;
    println!("[DEBUG] Entering KVM run loop");

    loop {
        loop_count += 1;
        println!("loop count: {}", loop_count);

        // KVM 싱글 스텝 활성화 (디버깅용)
        let guest_debug = kvm_bindings::kvm_guest_debug {
            control: 0x00000001,  //| 0x00000002, // ENABLE | SINGLESTEP
            ..Default::default() 
        };
        vm.vcpu_fd.set_guest_debug(&guest_debug).ok();

        // 1. 사전 상태 보정 및 로그
        let mut regs = vm.vcpu_fd.get_regs()?;
        {
            let rbp_prefix = regs.rbp >> 47;
            if rbp_prefix != 0 && rbp_prefix != 0x1FFFF {
                println!("[CPU] Correcting non-canonical RBP: 0x{:016x}", regs.rbp);
                regs.rbp = 0;
                vm.vcpu_fd.set_regs(&regs).ok();
            }
        }
        println!("[DEBUG] KVM Run Iteration: {} | Current RIP: 0x{:016x}", loop_count, regs.rip);

        // 2. 단일 실행 (The Only Run)
        let exit_reason = vm.vcpu_fd.run()?;
        
        println!("Exit Reason: {:?}, RIP: 0x{:x}", exit_reason, regs.rip);
        println!("[DEBUG] Iteration {} Exit: {:?}", loop_count, exit_reason);



        let action = match exit_reason {
            VcpuExit::IoOut(addr, data) => {
                if (addr == 0x3F8 || addr == 0xF8 || addr == 0x80) && !data.is_empty() {
                    if addr == 0x3F8 && !serial_detected {
                        println!("\n[SERIAL] Output detected!");
                        serial_detected = true;
                    }
                    print!("{}", data[0] as char);
                    io::stdout().flush()?;
                    None 
                } else if addr == 0xF9 && !data.is_empty() {
                    Some(LoopAction::Trap(data[0]))
                } else {
                    None
                }
                
            }
            VcpuExit::IoIn(addr, data) => {
                if !data.is_empty() {
                    match addr {
                        0x3FD => data[0] = 0x20, // LSR: Ready
                        0x3FE => data[0] = 0xB0, // MSR: Connected
                        _ => {}
                    }
                }
                None
            }
            // [FIX] Intercepted Software Breakpoints (KVM_SET_GUEST_DEBUG)
            VcpuExit::Debug(_) => {
                Some(LoopAction::GuestDebug)
            }
            VcpuExit::Hlt => Some(LoopAction::Dump("HLT".to_string())),
            VcpuExit::Shutdown => Some(LoopAction::Dump("Shutdown (Triple Fault)".to_string())),
            other => Some(LoopAction::Dump(format!("{:?}", other))),
        };
        

        sync_kernel_state(vm).ok();

        match action {
            Some(LoopAction::Trap(v)) => {
                if let Err(e) = debug::handle_diagnostic_trap(vm, v) {
                    return Err(e);
                }
            }
            Some(LoopAction::GuestDebug) => {
                if let Err(e) = debug::handle_guest_debug(vm) {
                    return Err(e);
                }
            }
            Some(LoopAction::Dump(msg)) => {
                println!("\nKVM EXIT: {}", msg);
                return debug::dump_all_registers(vm);
            }
            None => continue,
        }
    }
}

/*fn sync_kernel_state(vm: &mut Vm) -> Result<(), Box<dyn std::error::Error>> {
    let mut msrs = Msrs::from_entries(&[
        kvm_msr_entry { index: 0xc0000101, ..Default::default() }, // GS_BASE
        kvm_msr_entry { index: 0xc0000102, ..Default::default() }, // KGS_BASE
    ]).unwrap();
    
    if vm.vcpu_fd.get_msrs(&mut msrs).is_ok() {
        let entries = msrs.as_slice();
        let gs = entries[0].data;
        let kgs = entries[1].data;
        let kpcr_vaddr = 0xFFFFF80004010000;

        let gs_bad = (gs != 0) && (gs < 0xFFFF800000000000);
        let kgs_bad = (kgs != 0) && (kgs < 0xFFFF800000000000);

        if gs_bad || kgs_bad {
            let new_msrs = Msrs::from_entries(&[
                kvm_msr_entry { index: 0xc0000101, data: if gs_bad { kpcr_vaddr } else { gs }, ..Default::default() },
                kvm_msr_entry { index: 0xc0000102, data: if kgs_bad { kpcr_vaddr } else { kgs }, ..Default::default() },
            ]).unwrap();
            vm.vcpu_fd.set_msrs(&new_msrs).ok();
        }
    }
    Ok(())
} */

fn sync_kernel_state(vm: &mut Vm) -> Result<(), Box<dyn std::error::Error>> {
    // 1. SREGS(Segment Registers)를 가져옵니다.
    // 여기서 CS(Code Segment) 정보를 확인할 수 있습니다.
    let sregs = vm.vcpu_fd.get_sregs()?;

    // 2. CPL (Current Privilege Level) 확인
    // sregs.cs.dpl 값이 0이면 커널 모드(Ring 0), 3이면 유저 모드(Ring 3)입니다.
    // 유저 모드일 경우, GS_BASE가 낮은 주소(User Memory)를 가리키는 것이 정상이므로
    // 검사 로직을 수행하지 않고 즉시 리턴합니다.
    if sregs.cs.dpl != 0 {
        return Ok(());
    }

    // --- 아래 로직은 오직 커널 모드(Ring 0)일 때만 실행됩니다 ---

    let mut msrs = Msrs::from_entries(&[
        kvm_msr_entry { index: 0xc0000101, ..Default::default() }, // GS_BASE
        kvm_msr_entry { index: 0xc0000102, ..Default::default() }, // KGS_BASE
    ]).unwrap();
    
    if vm.vcpu_fd.get_msrs(&mut msrs).is_ok() {
        let entries = msrs.as_slice();
        let gs = entries[0].data;
        let kgs = entries[1].data;
        
        // [설정 필요] 실제 KPCR이 매핑된 가상 주소
        let kpcr_vaddr = 0xFFFFF80004010000; 

        // 조건: 값이 0이 아니고(초기화 됨), 
        // AND Canonical Kernel Address(상위 비트가 FFFF...)가 아닌 경우
        // 즉, 커널 모드인데 GS가 이상한 물리 주소나 유저 주소를 가리키고 있을 때만
        let gs_bad = (gs != 0) && (gs < 0xFFFF800000000000);
        let kgs_bad = (kgs != 0) && (kgs < 0xFFFF800000000000);

        if gs_bad || kgs_bad {
            // 디버깅을 위해 로그를 남기는 것을 추천합니다.
            // println!("[HYPERVISOR] Fixing GS/KGS in Ring 0. GS: {:x}, KGS: {:x}", gs, kgs);

            let new_msrs = Msrs::from_entries(&[
                kvm_msr_entry { 
                    index: 0xc0000101, 
                    data: if gs_bad { kpcr_vaddr } else { gs }, 
                    ..Default::default() 
                },
                kvm_msr_entry { 
                    index: 0xc0000102, 
                    data: if kgs_bad { kpcr_vaddr } else { kgs }, 
                    ..Default::default() 
                },
            ]).unwrap();
            vm.vcpu_fd.set_msrs(&new_msrs).ok();
        }
    }
    Ok(())
}

enum LoopAction {
    Trap(u8),
    GuestDebug,
    Dump(String),
}

fn setup_long_mode(vm: &mut Vm, krnl_entry_v: u64, stack_v: u64, lpb_v: u64) -> Result<(), Box<dyn std::error::Error>> {
    let k_virt_base: u64 = 0xFFFFF80000000000;
    let gdt_pbase: u64 = crate::SYSTEM_BASE;
    let tss_pbase: u64 = gdt_pbase + 0x1000;
    let gdt_vbase = k_virt_base + gdt_pbase;
    let tss_vbase = k_virt_base + tss_pbase;
    let kpcr_vaddr: u64 = lpb_v + 0x10000; 
    let bridge_vaddr: u64 = 0xFFFFF80000000000 + crate::SYSTEM_BASE + 0x50000; 
    
    let mut cpuid = vm.kvm.get_supported_cpuid(kvm_bindings::KVM_MAX_CPUID_ENTRIES)?;
    for entry in cpuid.as_mut_slice() {
        if entry.function == 0x1 {
            // [STEALTH] Clear Hypervisor Present Bit (Bit 31 of ECX)
            entry.ecx &= !(1 << 31);
        }
        if entry.function == 0x40000000 {
            // [STEALTH] Hide "KVMKVMKVM" signature
            entry.ebx = 0; entry.ecx = 0; entry.edx = 0;
        }
        if entry.function == 0x80000001 { entry.edx |= 1 << 20; }
    }
    vm.vcpu_fd.set_cpuid2(&cpuid)?;

    let mut gdt: [u64; 32] = [0; 32];
    gdt[1] = 0x00af9a000000ffff; 
    gdt[2] = 0x00af9a000000ffff; // CS (0x10) 
    gdt[3] = 0x00cf92000000ffff; // SS (0x18) 
    gdt[4] = 0x00affb000000ffff; 
    gdt[5] = 0x00cff3000000ffff; // DS (0x2b) 
    gdt[10] = 0x00cff3000000ffff; // FS (0x53) 

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
    sregs.cr0 = (1 << 31) | (1 << 0) | (1 << 1) | (1 << 16) | (1 << 21); // bit21 WP=1 (Write Protect), bit16 NE=1 (Numeric Error), bit1 MP=1 (Monitor Coprocessor)
    sregs.cr0 |= (1 << 18);  // AM=1
    
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
    // [1순위 핵심 수정] Windows kernel startup convention: RCX = LoaderParameterBlock virtual addr
    regs.rcx = lpb_v;  // ← 이 줄이 없거나 0이면 kernel init에서 LPB 못 읽고 crash (나중 단계지만 지금은 long mode 진입 후 중요)
    regs.rdx = 0;      // 필요 시 다른 값, 보통 0

    vm.vcpu_fd.set_regs(&regs)?;

    // [1순위 수정] 설정 후 실제 KVM 상태 확인 (이 로그가 제대로 나오면 long mode 진입 성공 신호!)
    let sregs_after = vm.vcpu_fd.get_sregs()?;
    let regs_after = vm.vcpu_fd.get_regs()?;

    println!("\n[DEBUG] AFTER SET_SREGS & SET_REGS CHECK - MUST SEE THIS LOG!");
    println!("CR0:  0x{:016x} ", sregs_after.cr0);
    println!("CR4:  0x{:016x}  (bit5 PAE=1)", sregs_after.cr4);
    println!("EFER: 0x{:016x}  (bit8 LME=1, bit11 NXE=1)", sregs_after.efer);
    println!("CR3:  0x{:016x}  (PML4 phys addr 0x8002000)", sregs_after.cr3);
    println!("GDT base: 0x{:016x}  ", sregs_after.gdt.base);
    println!("IDT base: 0x{:016x}", sregs_after.idt.base);
    println!("CS: selector=0x{:x}, l=1 (long mode code segment)", sregs_after.cs.selector);
    println!("GS.base: 0x{:016x}  (KPCR virtual addr)", sregs_after.gs.base);
    println!("RIP: 0x{:016x}  (kernel entry point)", regs_after.rip);
    println!("RCX: 0x{:016x} ", regs_after.rcx);
    println!("RSP: 0x{:016x}", regs_after.rsp);

    Ok(())
}
