use crate::vm::Vm;
use kvm_ioctls::VcpuExit;
use kvm_bindings::Msrs;
use kvm_bindings::kvm_msr_entry;
use std::io::{self, Write};

pub fn run(vm: &mut Vm) -> Result<(), Box<dyn std::error::Error>> {
    setup_long_mode(vm)?;
    println!("\n--- KVM vCPU Start ---");

    loop {
        let exit_reason = vm.vcpu_fd.run()?;
        match exit_reason {
            VcpuExit::IoOut(addr, data) => {
                if (addr == 0x3F8 || addr == 0xF8) && !data.is_empty() {
                    print!("{}", data[0] as char);
                    io::stdout().flush()?;
                } else if addr == 0xF9 && !data.is_empty() {
                    println!("\n\n[DIAGNOSTIC] Trap triggered! Vector Number: {}", data[0]);
                    dump_all_registers(vm)?;
                    break;
                }
            }
            VcpuExit::Hlt => {
                println!("\nKVM: HLT Executed.");
                dump_all_registers(vm)?;
                break;
            }
            VcpuExit::Shutdown => {
                println!("\nKVM: Shutdown (Triple Fault) detected!");
                dump_all_registers(vm)?;
                break;
            }
            other => {
                println!("KVM: Stopped ({:?})", other);
                dump_all_registers(vm)?;
                break;
            }
        }
    }
    Ok(())
}

fn dump_all_registers(vm: &mut Vm) -> Result<(), Box<dyn std::error::Error>> {
    let regs = vm.vcpu_fd.get_regs()?;
    let sregs = vm.vcpu_fd.get_sregs()?;
    
    let mut msrs = Msrs::from_entries(&[
        kvm_msr_entry { index: 0xc0000101, ..Default::default() }, // GS_BASE
        kvm_msr_entry { index: 0xc0000102, ..Default::default() }, // KERNEL_GS_BASE
    ]).unwrap();
    vm.vcpu_fd.get_msrs(&mut msrs).ok();

    println!("------------------ CPU FULL DUMP ------------------");
    println!("RIP: 0x{:016x}  RSP: 0x{:016x}", regs.rip, regs.rsp);
    println!("RAX: 0x{:016x}  RCX: 0x{:016x}", regs.rax, regs.rcx);
    println!("RDX: 0x{:016x}  RBX: 0x{:016x}", regs.rdx, regs.rbx);
    println!("RSI: 0x{:016x}  RDI: 0x{:016x}", regs.rsi, regs.rdi);
    println!("CR2: 0x{:016x}  CR3: 0x{:016x}", sregs.cr2, sregs.cr3);
    println!("GS_BASE: 0x{:016x}", msrs.as_slice()[0].data);
    println!("KGS_BASE: 0x{:016x}", msrs.as_slice()[1].data);
    println!("CS: 0x{:x}  SS: 0x{:x}  EFER: 0x{:x}", sregs.cs.selector, sregs.ss.selector, sregs.efer);
    println!("---------------------------------------------------");
    Ok(())
}

fn setup_long_mode(vm: &mut Vm) -> Result<(), Box<dyn std::error::Error>> {
    let gdt_base: usize = 0x8000000; // 128MB로 이동
    let tss_base: u64 = 0x8000200;
    let kpcr_vaddr: u64 = 0xFFFFF80006000000; // main.rs와 동기화
    
    let mut gdt: [u64; 32] = [0; 32];
    gdt[1] = 0x00cf9a000000ffff; 
    gdt[2] = 0x00af9a000000ffff; 
    gdt[3] = 0x00cf92000000ffff; 
    gdt[4] = 0x00cffb000000ffff; 
    gdt[5] = 0x00cff3000000ffff; 
    gdt[6] = 0x00affb000000ffff; 
    gdt[7] = 0x00cff3000000ffff; 
    let tss_limit = 104 - 1;
    gdt[8] = (tss_base & 0xffffff) << 16 | (tss_base & 0xff000000) << 32 | 0x0000890000000000 | tss_limit;
    gdt[9] = tss_base >> 32;
    gdt[10] = 0x00cff2000000ffff; 
    gdt[11] = 0x00cff2000000ffff;

    let mut gdt_bytes = Vec::new();
    for entry in &gdt { gdt_bytes.extend_from_slice(&entry.to_le_bytes()); }
    vm.write_memory(gdt_base, &gdt_bytes)?;
    vm.write_memory(tss_base as usize, &[0u8; 104])?;

    vm.vcpu_fd.set_fpu(&kvm_bindings::kvm_fpu::default())?;
    let mut sregs = vm.vcpu_fd.get_sregs()?;

    sregs.cr3 = (gdt_base + 0x1000) as u64; // PML4: 128MB + 4KB
    sregs.cr4 = (1 << 5) | (1 << 9) | (1 << 10) | (1 << 16); 
    sregs.efer = (1 << 0) | (1 << 8) | (1 << 10) | (1 << 11); 
    sregs.cr0 = (1 << 31) | (1 << 0) | (1 << 1) | (1 << 16); 

    sregs.gdt.base = gdt_base as u64;
    sregs.gdt.limit = (std::mem::size_of::<u64>() * 32 - 1) as u16;

    fn seg_64(selector: u16, is_code: bool) -> kvm_bindings::kvm_segment {
        kvm_bindings::kvm_segment {
            base: 0, limit: 0xffffffff, selector, present: 1,
            type_: if is_code { 11 } else { 3 },
            s: 1, l: if is_code { 1 } else { 0 }, g: 1,
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
        type_: 11, present: 1, s: 0, g: 0, ..kvm_bindings::kvm_segment::default()
    };

    vm.vcpu_fd.set_sregs(&sregs)?;

    let msr_entries = [
        kvm_msr_entry { index: 0xc0000080, data: sregs.efer, ..Default::default() }, 
        kvm_msr_entry { index: 0xc0000081, data: 0x001B001000000000, ..Default::default() }, 
        kvm_msr_entry { index: 0xc0000101, data: kpcr_vaddr, ..Default::default() }, 
        kvm_msr_entry { index: 0xc0000102, data: kpcr_vaddr, ..Default::default() }, 
        kvm_msr_entry { index: 0x277, data: 0x0007040600070406, ..Default::default() }, 
    ];
    let msrs = Msrs::from_entries(&msr_entries).unwrap();
    vm.vcpu_fd.set_msrs(&msrs).ok();

    let mut regs = vm.vcpu_fd.get_regs()?;
    regs.rflags = 0x2;
    vm.vcpu_fd.set_regs(&regs)?;

    Ok(())
}
