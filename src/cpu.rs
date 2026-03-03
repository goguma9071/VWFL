use crate::vm::Vm;
use kvm_ioctls::VcpuExit;
use kvm_bindings::{kvm_msr_entry, Msrs};
use crate::debug::dump_all_registers;
use std::io::{self, Write};
use crate::debug;

// [CORE FIX] 하이퍼바이저 수준의 최소 APIC 상태 저장소
struct ApicState {
    tpr: u32,
    svr: u32,
    lvt_timer: u32,
    init_count: u32,
}

static mut APIC: ApicState = ApicState {
    tpr: 0,
    svr: 0x1FF, // 기본값: SVR Enabled
    lvt_timer: 0x10000, // Masked
    init_count: 0,
};

pub fn run(vm: &mut Vm, krnl_entry_v: u64, stack_v: u64, lpb_v: u64) -> Result<(), Box<dyn std::error::Error>> {
    println!("[CPU] Initializing vCPU state...");
    setup_long_mode(vm, krnl_entry_v, stack_v, lpb_v)?;

    let k = false; // true: 디버그 모드, false: 고속 모드
    let debug_control = if k { 0x00000001 | 0x00000002 } else { 0x00000001 };
    vm.vcpu_fd.set_guest_debug(&kvm_bindings::kvm_guest_debug {
        control: debug_control,
        ..Default::default() 
    }).ok();

    println!("\n--- KVM vCPU Start (Debug Mode: {}) ---", k);

    let mut loop_count: u64 = 0;
    let mut last_rip: u64 = 0;
    let mut hang_count: u32 = 0;

    loop {
        loop_count += 1;
        
        {
            let mut kvm_run = vm.vcpu_fd.get_kvm_run();
            if kvm_run.ready_for_interrupt_injection == 0 {
                kvm_run.request_interrupt_window = 1;
            } else {
                kvm_run.request_interrupt_window = 0;
            }
        }

        // KUSER_SHARED_DATA 시간 업데이트
        if loop_count % 500 == 0 {
            let kuser_p = crate::KUSER_PBASE;
            let virtual_time = loop_count.wrapping_mul(10000); 
            let tick_count = (loop_count / 100) as u32;
            vm.write_memory((kuser_p + 0x08) as usize, &virtual_time.to_le_bytes()).ok();
            vm.write_memory((kuser_p + 0x18) as usize, &virtual_time.to_le_bytes()).ok();
            vm.write_memory((kuser_p + 0x320) as usize, &tick_count.to_le_bytes()).ok();
        }

        let exit_reason = vm.vcpu_fd.run()?; 

        let action = match exit_reason {
            VcpuExit::IrqWindowOpen => LoopAction::Trace,
            VcpuExit::IoOut(addr, data) => {
                let val = if data.is_empty() { 0 } else { data[0] };
                if addr == 0xF9 { LoopAction::Trap(val) }
                else if addr == 0x3F8 || addr == 0xF8 || addr == 0x80 { LoopAction::SerialOut(val) }
                else { LoopAction::LogIoOut(addr, val) }
            }
            VcpuExit::IoIn(addr, data) => {
                if !data.is_empty() {
                    match addr {
                        0x3F8 => data[0] = 0,
                        0x3FA => data[0] = 1,
                        0x3FD => data[0] = 0x60,
                        0x3FE => data[0] = 0xB0,
                        _ => data[0] = 0,
                    }
                }
                LoopAction::LogIoIn(addr)
            }
            VcpuExit::MmioRead(addr, data) => {
                let mut val = 0u32;
                let mut is_apic = false;

                if addr >= 0xfee00000 && addr <= 0xfee00fff {
                    is_apic = true;
                    unsafe {
                        val = match addr & 0xFFF {
                            0x20 => 0x0,      // APIC ID
                            0x30 => 0x50014,  // VERSION
                            0x80 => APIC.tpr, // TPR (Task Priority)
                            0xF0 => APIC.svr, // SVR (Spurious Vector)
                            0x320 => APIC.lvt_timer,
                            0x380 => APIC.init_count,
                            0x390 => {
                                // [CORE FIX] 타이머 카운트다운 시뮬레이션
                                // 초기값에서 loop_count의 일부를 뺀 값을 주어 숫자가 줄어들게 만듭니다.
                                if APIC.init_count > 0 {
                                    APIC.init_count.saturating_sub((loop_count % 1000) as u32)
                                } else { 0 }
                            },
                            _ => 0,
                        };
                    }
                }

                let bytes = val.to_le_bytes();
                let len = data.len().min(4);
                data[..len].copy_from_slice(&bytes[..len]);

                if is_apic { LoopAction::LogApic(addr, val, false) }
                else { LoopAction::LogMmioRead(addr) }
            }
            VcpuExit::MmioWrite(addr, data) => {
                let val = if data.len() >= 4 { u32::from_le_bytes(data[0..4].try_into().unwrap()) } 
                          else if data.len() >= 1 { data[0] as u32 } else { 0 };
                
                if addr >= 0xfee00000 && addr <= 0xfee00fff {
                    unsafe {
                        match addr & 0xFFF {
                            0x80  => APIC.tpr = val,
                            0xF0  => APIC.svr = val,
                            0x320 => APIC.lvt_timer = val,
                            0x380 => APIC.init_count = val,
                            _ => {}
                        }
                    }
                    LoopAction::LogApic(addr, val, true)
                } else {
                    LoopAction::LogMmioWrite(addr, val)
                }
            }
            VcpuExit::Debug(_) => LoopAction::Trace,
            VcpuExit::Hlt => LoopAction::Dump("HLT (CPU Idle)".to_string()),
            VcpuExit::Shutdown => LoopAction::Dump("Shutdown (Triple Fault)".to_string()),
            other => LoopAction::LogOther(format!("{:?}", other)),
        };

        match action {
            LoopAction::Trace if k && loop_count % 50 == 0 => {
                let regs = vm.vcpu_fd.get_regs().ok();
                println!("[TRACE] RIP: 0x{:016x?} | Count: {}", regs.as_ref().map(|r| r.rip), loop_count);
            }
            LoopAction::SerialOut(c) => {
                print!("{}", c as char);
                io::stdout().flush()?;
            }
            LoopAction::LogMmioRead(addr) => {
                if !k && loop_count % 1000 == 0 { println!("[MMIO READ ] Addr: 0x{:x}", addr); }
            }
            LoopAction::LogMmioWrite(addr, val) => {
                if !k && loop_count % 1000 == 0 { println!("[MMIO WRITE] Addr: 0x{:x}, Val: 0x{:x}", addr, val); }
            }
            LoopAction::LogApic(addr, val, is_write) => {
                if loop_count % 100 == 0 {
                    let reg_name = match addr & 0xFFF {
                        0x20 => "ID", 0x30 => "VER", 0x80 => "TPR", 0xB0 => "EOI", 0xF0 => "SVR",
                        0x320 => "LVT_TMR", 0x380 => "INIT_CNT", 0x390 => "CUR_CNT", _ => "UNK",
                    };
                    if is_write { println!("[APIC W] {}: 0x{:x}", reg_name, val); }
                    else { println!("[APIC R] {}: 0x{:x}", reg_name, val); }
                }
            }
            LoopAction::Trap(v) => debug::handle_diagnostic_trap(vm, v)?,
            LoopAction::Dump(msg) => {
                println!("\nKVM EXIT: {}", msg);
                return debug::dump_all_registers(vm);
            }
            _ => {}
        }

        if loop_count % 50000 == 0 {
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
    LogMmioRead(u64),
    LogMmioWrite(u64, u32),
    LogOther(String),
    Trap(u8),
    Dump(String),
    LogApic(u64, u32, bool),
}

fn setup_long_mode(vm: &mut Vm, krnl_entry_v: u64, stack_v: u64, lpb_v: u64) -> Result<(), Box<dyn std::error::Error>> {
    let k_virt_base: u64 = 0xFFFFF80000000000;
    let gdt_pbase: u64 = crate::SYSTEM_BASE;
    let tss_pbase: u64 = gdt_pbase + 0x1000;
    let gdt_vbase = k_virt_base + gdt_pbase;
    let tss_vbase = k_virt_base + tss_pbase;
    let kpcr_vaddr: u64 = lpb_v + 0x10000; 
    
    let mut cpuid = vm.kvm.get_supported_cpuid(kvm_bindings::KVM_MAX_CPUID_ENTRIES)?;
    for entry in cpuid.as_mut_slice() {
        if entry.function == 0x1 { 
            entry.ecx &= !(1 << 31); // Hypervisor present = 0
            entry.ecx &= !(1 << 21); // x2APIC = 0
        }
        if entry.function == 0x40000000 { entry.ebx = 0; entry.ecx = 0; entry.edx = 0; }
    }
    vm.vcpu_fd.set_cpuid2(&cpuid)?;

    // GDT/TSS/Sregs 설정 (기존과 동일하게 유지하되 명세에 맞춤)
    let mut gdt: [u64; 32] = [0; 32];
    gdt[1] = 0x00af9a000000ffff; 
    gdt[2] = 0x00af9a000000ffff; 
    gdt[3] = 0x00cf92000000ffff; 
    gdt[4] = 0x00affb000000ffff; 
    gdt[5] = 0x00cff3000000ffff; 
    
    // [CORE FIX] GDT Index 10 (Selector 0x50) 설정
    // Windows 커널 초기화 시 필수적인 User Data Segment
    gdt[10] = 0x00cff3000000ffff; 

    let tss_limit = 104 - 1;
    let tss_low = (tss_vbase & 0xffffff) << 16 | (tss_vbase & 0xff000000) << 32 | 0x0000890000000000 | tss_limit;
    let tss_high = tss_vbase >> 32;
    gdt[8] = tss_low; gdt[9] = tss_high;
    let mut gdt_bytes = Vec::new();
    for entry in &gdt { gdt_bytes.extend_from_slice(&entry.to_le_bytes()); }
    vm.write_memory(gdt_pbase as usize, &gdt_bytes)?;

    let mut tss = [0u8; 104];
    tss[4..12].copy_from_slice(&stack_v.to_le_bytes()); 
    vm.write_memory(tss_pbase as usize, &tss)?;

    let mut sregs = vm.vcpu_fd.get_sregs()?;
    sregs.cr3 = gdt_pbase + 0x100000 + 0x2000;
    sregs.cr4 = (1 << 5) | (1 << 7) | (1 << 9) | (1 << 10) | (1 << 16); 
    sregs.efer = (1 << 0) | (1 << 8) | (1 << 10) | (1 << 11); 
    sregs.cr0 = (1 << 31) | (1 << 0) | (1 << 1) | (1 << 5) | (1 << 16) | (1 << 18);
    sregs.gdt.base = gdt_vbase;
    sregs.gdt.limit = (32 * 8 - 1) as u16;
    sregs.idt.base = k_virt_base + gdt_pbase + 0x20000; 
    sregs.idt.limit = 0x0FFF;

    fn seg_64(selector: u16, is_code: bool) -> kvm_bindings::kvm_segment {
        kvm_bindings::kvm_segment {
            base: 0, limit: 0xffffffff, selector, present: 1,
            type_: if is_code { 11 } else { 3 },
            s: 1, l: if is_code { 1 } else { 0 }, g: 1, db: 0, dpl: 0,
            ..kvm_bindings::kvm_segment::default()
        }
    }
    sregs.cs = seg_64(0x10, true);
    let ds = seg_64(0x18, false);
    sregs.ds = ds; sregs.es = ds; sregs.ss = ds; sregs.gs = ds;
    sregs.gs.base = kpcr_vaddr;
    sregs.tr = kvm_bindings::kvm_segment { base: tss_vbase, limit: tss_limit as u32, selector: 0x40, type_: 9, present: 1, s: 0, g: 0, dpl: 0, ..kvm_bindings::kvm_segment::default() };
    vm.vcpu_fd.set_sregs(&sregs)?;

    let msr_entries = [
        kvm_msr_entry { index: 0xc0000080, data: sregs.efer, ..Default::default() }, 
        kvm_msr_entry { index: 0xc0000101, data: kpcr_vaddr, ..Default::default() }, 
        kvm_msr_entry { index: 0x1b, data: 0xfee00000 | 0x900, ..Default::default() }, 
    ];
    vm.vcpu_fd.set_msrs(&Msrs::from_entries(&msr_entries).unwrap()).ok();

    let mut regs = vm.vcpu_fd.get_regs()?;
    regs.rip = krnl_entry_v;
    regs.rsp = stack_v - 0x100;
    regs.rflags = 0x2;
    regs.rcx = lpb_v;
    regs.rdx = lpb_v; 
    vm.vcpu_fd.set_regs(&regs)?;
    Ok(())
}
