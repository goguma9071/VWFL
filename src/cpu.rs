use crate::vm::Vm;
use kvm_ioctls::VcpuExit;
use kvm_bindings::{kvm_msr_entry, Msrs};
use std::io::{self, Write};
use crate::debug;
use std::net::{TcpListener, TcpStream};
use gdbstub::stub::GdbStub;
use gdbstub::stub::run_blocking::{BlockingEventLoop, Event as GdbEvent, WaitForStopReasonError};
use gdbstub::stub::{BaseStopReason, DisconnectReason};
use crate::gdb::{VwflTarget, GdbResumeAction};
use gdbstub::common::Tid;
use std::marker::PhantomData;
use gdbstub::conn::{Connection, ConnectionExt};  

struct ApicState {
    tpr: u32,
    svr: u32,
    lvt_timer: u32,
    init_count: u32,
}

static mut APIC: ApicState = ApicState {
    tpr: 0,
    svr: 0x1FF, 
    lvt_timer: 0x10000, 
    init_count: 0,
};

pub fn run(vm: &mut Vm, krnl_entry_v: u64, stack_v: u64, lpb_v: u64) -> Result<(), Box<dyn std::error::Error>> {
    println!("[CPU] Initializing vCPU state...");
    setup_long_mode(vm, krnl_entry_v, stack_v, lpb_v)?;
    run_gdb_server(vm)
}

fn run_gdb_server(vm: &mut Vm) -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind("127.0.0.1:1234")?;
    println!("\n--- GDB Server Started ---");
    println!("Waiting for GDB connection on 127.0.0.1:1234...");
    
    let (stream, addr) = listener.accept()?;
    println!("GDB Client Connected: {}", addr);
    
    stream.set_nonblocking(true)?;

    let mut target = VwflTarget { vm, resume_action: None };
    let gdb = GdbStub::new(stream);

    match gdb.run_blocking::<VwflEventLoop<'_>>(&mut target)? {
        DisconnectReason::Disconnect => println!("[GDB] Disconnected."),
        DisconnectReason::Kill => println!("[GDB] Killed by client."),
        _ => println!("[GDB] Stopped."),
    }

    Ok(())
}

struct VwflEventLoop<'a> {
    _phantom: PhantomData<&'a ()>,
}

impl<'a> BlockingEventLoop for VwflEventLoop<'a> {
    type Target = VwflTarget<'a>;
    type Connection = TcpStream;
    type StopReason = BaseStopReason<Tid, u64>;

    fn wait_for_stop_reason(
        target: &mut Self::Target,
        conn: &mut Self::Connection,
    ) -> Result<GdbEvent<Self::StopReason>, WaitForStopReasonError<&'static str, std::io::Error>> {
        let mut loop_count: u64 = 0;
        
        loop {
            loop_count += 1;
            
            // [FIX] gdbstub::conn::Connection 규격에 맞게 수정
            match conn.peek().map_err(WaitForStopReasonError::Connection)? {
                Some(byte) => {
                    // 데이터가 있으면 읽어서 소모하고 IncomingData로 보고
                    let _ = conn.read().map_err(WaitForStopReasonError::Connection)?;
                    return Ok(GdbEvent::IncomingData(byte));
                }
                None => {}
            }

            if loop_count % 1000 == 0 {
                update_windows_time(target.vm, loop_count);
            }

            {
                let mut kvm_run = target.vm.vcpu_fd.get_kvm_run();
                kvm_run.request_interrupt_window = 1; 
            }

            let exit = target.vm.vcpu_fd.run().map_err(|_| WaitForStopReasonError::Target("KVM Run Error"))?;

            match exit {
                VcpuExit::Debug(_) => {
                    return Ok(GdbEvent::TargetStopped(BaseStopReason::DoneStep));
                }
                VcpuExit::IrqWindowOpen => {
                    continue;
                }
                VcpuExit::IoOut(addr, data) => {
                    let val = data[0];
                    if addr == 0xF9 { 
                        debug::handle_diagnostic_trap(target.vm, val).ok();
                        return Ok(GdbEvent::TargetStopped(BaseStopReason::Signal(gdbstub::common::Signal::SIGTRAP)));
                    }
                    if addr == 0x3F8 { print!("{}", val as char); io::stdout().flush().ok(); }
                }
                VcpuExit::MmioRead(addr, data) => {
                    handle_mmio_read(addr, data, loop_count);
                }
                VcpuExit::MmioWrite(addr, data) => {
                    handle_mmio_write(addr, data);
                }
                VcpuExit::Hlt => continue,
                VcpuExit::Shutdown => {
                    return Ok(GdbEvent::TargetStopped(BaseStopReason::Signal(gdbstub::common::Signal::SIGSEGV)));
                }
                _ => {
                    return Ok(GdbEvent::TargetStopped(BaseStopReason::Signal(gdbstub::common::Signal::SIGTRAP)));
                }
            }
        }
    }

    fn on_interrupt(_target: &mut Self::Target) -> Result<Option<Self::StopReason>, &'static str> {
        Ok(Some(BaseStopReason::Signal(gdbstub::common::Signal::SIGINT)))
    }
}

fn update_windows_time(vm: &mut Vm, count: u64) {
    let kuser_p = 0x9000000;
    let virtual_time = count.wrapping_mul(10000); 
    let time_bytes = virtual_time.to_le_bytes();
    let high_bytes = (virtual_time >> 32) as u32;
    vm.write_memory((kuser_p + 0x08) as usize, &time_bytes).ok(); 
    vm.write_memory((kuser_p + 0x10) as usize, &high_bytes.to_le_bytes()).ok(); 
    vm.write_memory((kuser_p + 0x14) as usize, &time_bytes).ok(); 
    vm.write_memory((kuser_p + 0x1C) as usize, &high_bytes.to_le_bytes()).ok(); 
}

fn handle_mmio_read(addr: u64, data: &mut [u8], loop_count: u64) {
    if addr >= 0xfee00000 && addr <= 0xfee00fff {
        unsafe {
            let val = match addr & 0xFFF {
                0x20 => 0x0, 0x30 => 0x50014, 0x80 => APIC.tpr, 0xF0 => APIC.svr,
                0x320 => APIC.lvt_timer, 0x380 => APIC.init_count,
                0x390 => if APIC.init_count > 0 { APIC.init_count.wrapping_sub((loop_count & 0xFFFF) as u32) } else { 0x100000 },
                _ => 0,
            };
            let bytes = val.to_le_bytes();
            let len = data.len().min(4);
            data[..len].copy_from_slice(&bytes[..len]);
        }
    }
}

fn handle_mmio_write(addr: u64, data: &[u8]) {
    if addr >= 0xfee00000 && addr <= 0xfee00fff {
        unsafe {
            let val = u32::from_le_bytes(data[0..4].try_into().unwrap_or([0;4]));
            match addr & 0xFFF {
                0x80 => APIC.tpr = val, 0xF0 => APIC.svr = val,
                0x320 => APIC.lvt_timer = val, 0x380 => APIC.init_count = val,
                _ => {}
            }
        }
    }
}

fn setup_long_mode(vm: &mut Vm, krnl_entry_v: u64, stack_v: u64, lpb_v: u64) -> Result<(), Box<dyn std::error::Error>> {
    let k_virt_base: u64 = 0xFFFFF80000000000;
    let gdt_pbase: u64 = 0x8000000;
    let tss_pbase: u64 = gdt_pbase + 0x1000;
    let gdt_vbase = k_virt_base + gdt_pbase;
    let tss_vbase = k_virt_base + tss_pbase;
    let kpcr_vaddr: u64 = lpb_v + 0x10000; 
    
    let mut cpuid = vm.kvm.get_supported_cpuid(kvm_bindings::KVM_MAX_CPUID_ENTRIES)?;
    for entry in cpuid.as_mut_slice() {
        if entry.function == 0x1 { entry.ecx &= !(1 << 31); entry.ecx &= !(1 << 21); }
        if entry.function == 0x40000000 { entry.ebx = 0; entry.ecx = 0; entry.edx = 0; }
    }
    vm.vcpu_fd.set_cpuid2(&cpuid)?;

    let mut gdt: [u64; 32] = [0; 32];
    gdt[1] = 0x00af9a000000ffff; gdt[2] = 0x00af9a000000ffff; gdt[3] = 0x00cf92000000ffff; 
    gdt[4] = 0x00affb000000ffff; gdt[5] = 0x00cff3000000ffff; gdt[10] = 0x00cff3000000ffff; 

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
