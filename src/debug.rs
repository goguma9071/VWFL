use crate::vm::Vm;
use kvm_bindings::{kvm_regs, kvm_msr_entry, Msrs};

pub fn dump_all_registers(vm: &mut Vm) -> Result<(), Box<dyn std::error::Error>> {
    let regs = vm.vcpu_fd.get_regs()?;
    let sregs = vm.vcpu_fd.get_sregs()?;

    println!("------------------ CPU FULL DUMP ------------------");
    println!("RIP: 0x{:016x}  RSP: 0x{:016x}  RBP: 0x{:016x}", regs.rip, regs.rsp, regs.rbp);
    println!("RAX: 0x{:016x}  RCX: 0x{:016x}  RDX: 0x{:016x}", regs.rax, regs.rcx, regs.rdx);
    println!("RBX: 0x{:016x}  RSI: 0x{:016x}  RDI: 0x{:016x}", regs.rbx, regs.rsi, regs.rdi);
    println!("R8 : 0x{:016x}  R9 : 0x{:016x}  R10: 0x{:016x}", regs.r8, regs.r9, regs.r10);
    println!("R11: 0x{:016x}  R12: 0x{:016x}  R13: 0x{:016x}", regs.r11, regs.r12, regs.r13);
    println!("R14: 0x{:016x}  R15: 0x{:016x}", regs.r14, regs.r15);
    
    println!("CR0: 0x{:016x}  CR2: 0x{:016x}  CR3: 0x{:016x}", sregs.cr0, sregs.cr2, sregs.cr3);
    println!("CR4: 0x{:016x}  EFER: 0x{:016x}", sregs.cr4, sregs.efer);
    
    println!("GDT: Base=0x{:016x} Limit=0x{:04x}", sregs.gdt.base, sregs.gdt.limit);
    println!("IDT: Base=0x{:016x} Limit=0x{:04x}", sregs.idt.base, sregs.idt.limit);
    println!("TR : Base=0x{:016x} Limit=0x{:08x} Type={}", sregs.tr.base, sregs.tr.limit, sregs.tr.type_);

    let mut msrs = Msrs::from_entries(&[
        kvm_msr_entry { index: 0xc0000101, ..Default::default() },
        kvm_msr_entry { index: 0xc0000102, ..Default::default() },
        kvm_msr_entry { index: 0xc0000082, ..Default::default() }, // LSTAR
    ]).unwrap();
    
    let mut gs_base = 0u64;
    let mut kgs_base = 0u64;
    let mut lstar = 0u64;
    if let Ok(_) = vm.vcpu_fd.get_msrs(&mut msrs) {
        let entries = msrs.as_slice();
        gs_base = entries[0].data;
        kgs_base = entries[1].data;
        lstar = entries[2].data;
    }

    println!("GS_BASE: 0x{:016x}  KGS_BASE: 0x{:016x}", gs_base, kgs_base);
    println!("LSTAR  : 0x{:016x}", lstar);
    println!("CS: 0x{:x}  SS: 0x{:x}  DS: 0x{:x}", sregs.cs.selector, sregs.ss.selector, sregs.ds.selector);
    println!("---------------------------------------------------");
    Ok(())
}

pub fn handle_diagnostic_trap(vm: &mut Vm, vector: u8) -> Result<(), Box<dyn std::error::Error>> {
    let phys_base = 0x70000;
    let read_phys_u64 = |paddr: u64| -> u64 {
        unsafe {
            let ptr = vm.mem_ptr.add(paddr as usize);
            u64::from_le_bytes(*(ptr as *const [u8; 8]))
        }
    };
    let write_phys_u64 = |paddr: u64, val: u64| {
        unsafe {
            let ptr = vm.mem_ptr.add(paddr as usize);
            *(ptr as *mut u64) = val;
        }
    };

    let regs = vm.vcpu_fd.get_regs()?;
    let rsp_phys = virt_to_phys(regs.rsp);

    // [SPECIAL] Skip Vector 3 (Breakpoint)
    if vector == 3 {
        let current_rip = read_phys_u64(rsp_phys);
        let old_rflags = read_phys_u64(rsp_phys + 16);
        let old_rsp = read_phys_u64(rsp_phys + 24);
        
        println!("[DEBUG] Breakpoint (Vector 3) triggered at RIP: 0x{:016x}. Resuming...", current_rip);
        
        let mut regs = vm.vcpu_fd.get_regs()?;
        regs.rip = current_rip + 1; // INT 3 is 1 byte (0xCC)
        regs.rax = read_phys_u64(phys_base);
        regs.rcx = read_phys_u64(phys_base + 8);
        regs.rdx = read_phys_u64(phys_base + 16);
        regs.rsp = old_rsp;
        regs.rflags = old_rflags;
        vm.vcpu_fd.set_regs(&regs)?;
        return Ok(());
    }

    // [SPECIAL] Handle INT 0x2d (Debug Service)
    if vector == 0x2d {
        let current_rip = read_phys_u64(rsp_phys);
        let old_rflags = read_phys_u64(rsp_phys + 16);
        let old_rsp = read_phys_u64(rsp_phys + 24);
        
        println!("[DEBUG] Debug Service (INT 0x2d) triggered at RIP: 0x{:016x}. Skipping...", current_rip);
        
        let mut regs = vm.vcpu_fd.get_regs()?;
        regs.rip = current_rip + 2; // INT 0x2d is 2 bytes (0xCD 0x2D)
        regs.rax = read_phys_u64(phys_base);
        regs.rcx = read_phys_u64(phys_base + 8);
        regs.rdx = read_phys_u64(phys_base + 16);
        regs.rsp = old_rsp;
        regs.rflags = old_rflags;
        vm.vcpu_fd.set_regs(&regs)?;
        return Ok(());
    }

    if vector == 13 {
        println!("\n[FATAL] General Protection Fault (#GP) detected!");
        // #GP는 무시하면 무한 루프에 빠질 가능성이 매우 높으므로 덤프 출력 후 종료 유도
    }

    println!("\n[DIAGNOSTIC] Trap triggered! Vector Number: {}", vector);
    
    println!("------------------ EXCEPTION FRAME ------------------");

    let has_error_code = matches!(vector, 8 | 10 | 11 | 12 | 13 | 14 | 17);
    let faulting_rip = if has_error_code {
        let err = read_phys_u64(rsp_phys);
        let rip = read_phys_u64(rsp_phys + 8);
        println!("ERROR CODE  : 0x{:x}", err);
        println!("FAULTING RIP: 0x{:016x}", rip);
        rip
    } else {
        let rip = read_phys_u64(rsp_phys);
        println!("FAULTING RIP: 0x{:016x}", rip);
        rip
    };

    let rip_phys = virt_to_phys(faulting_rip);
    println!("CODE AT RIP (Phys: 0x{:x}): ", rip_phys);
    hex_dump_bytes(vm, rip_phys, 16);

    println!("STACK DUMP (Phys: 0x{:x}): ", rsp_phys);
    hex_dump_bytes(vm, rsp_phys, 64);

    println!("KERNEL RAX: 0x{:016x}", read_phys_u64(phys_base));
    println!("KERNEL RCX: 0x{:016x}", read_phys_u64(phys_base + 8));
    println!("KERNEL RDX: 0x{:016x}", read_phys_u64(phys_base + 16));
    
    dump_all_registers(vm)
}

fn virt_to_phys(vaddr: u64) -> u64 {
    if vaddr >= 0xFFFFF80000000000 {
        vaddr - 0xFFFFF80000000000
    } else if vaddr >= 0x140000000 && vaddr < 0x180000000 {
        (vaddr - 0x140000000) + 0x200000
    } else {
        vaddr & (crate::vm::MEM_SIZE as u64 - 1)
    }
}

fn hex_dump_bytes(vm: &Vm, paddr: u64, size: usize) {
    if paddr as usize + size > crate::vm::MEM_SIZE {
        println!("  [Error] Address 0x{:x} out of bounds.", paddr);
        return;
    }
    unsafe {
        let ptr = vm.mem_ptr.add(paddr as usize);
        for i in 0..size {
            print!("{:02x} ", *ptr.add(i));
            if (i + 1) % 16 == 0 { println!(); }
        }
        if size % 16 != 0 { println!(); }
    }
}

pub fn hex_dump(vm: &Vm, paddr: u64, size: usize) {
    println!("--- HEX DUMP (Phys: 0x{:x}, Size: {}) ---", paddr, size);
    hex_dump_bytes(vm, paddr, size);
    println!("-----------------------------------------");
}

pub fn setup_diagnostic_idt(vm: &mut Vm) -> Result<(), Box<dyn std::error::Error>> {
    let idt_pbase = crate::SYSTEM_BASE + 0x20000; // [FIX] IDT physical base
    let stub_base_p = crate::SYSTEM_BASE + 0x10000; 
    let stub_base_v = 0xFFFFF80000000000 + stub_base_p; 
    let save_area: u64 = 0x70000; 

    for i in 0..256 {
        // ... (stub 생략은 동일)
        let mut stub = Vec::new();
        stub.extend_from_slice(&[0x48, 0xA3]); stub.extend_from_slice(&save_area.to_le_bytes()); 
        stub.extend_from_slice(&[0x48, 0x89, 0xC8, 0x48, 0xA3]); stub.extend_from_slice(&(save_area + 8).to_le_bytes()); 
        stub.extend_from_slice(&[0x48, 0x89, 0xD0, 0x48, 0xA3]); stub.extend_from_slice(&(save_area + 16).to_le_bytes()); 
        stub.extend_from_slice(&[0xB0, i as u8, 0xE6, 0xF9, 0xF4]); 
        
        vm.write_memory((stub_base_p + i as u64 * 64) as usize, &stub).map_err(|e| e.to_string())?;

        let mut entry = [0u8; 16];
        let h = stub_base_v + i as u64 * 64; 
        entry[0..2].copy_from_slice(&(h as u16).to_le_bytes()); 
        entry[2..4].copy_from_slice(&0x10u16.to_le_bytes());    
        entry[5] = 0xEE; // [FIX] Present=1, DPL=3, Type=Interrupt Gate (0x0E) -> 0xEE
        entry[6..8].copy_from_slice(&((h >> 16) as u16).to_le_bytes()); 
        entry[8..12].copy_from_slice(&((h >> 32) as u32).to_le_bytes()); 
        vm.write_memory((idt_pbase + i as u64 * 16) as usize, &entry).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn verify_mapping(_vm: &Vm, _v: u64) {}
