use crate::vm::Vm;
use crate::SYSTEM_BASE;
use crate::KRNL_PBASE;
use crate::HAL_PBASE;
use crate::STACK_PBASE;
use crate::KUSER_PBASE;
use crate::MEM_SIZE;
use kvm_bindings::{kvm_msr_entry, Msrs};

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
        kvm_msr_entry { index: 0xc0000082, ..Default::default() }, 
    ]).unwrap();
    
    if let Ok(_) = vm.vcpu_fd.get_msrs(&mut msrs) {
        let entries = msrs.as_slice();
        println!("GS_BASE: 0x{:016x}  KGS_BASE: 0x{:016x}", entries[0].data, entries[1].data);
        println!("LSTAR  : 0x{:016x}", entries[2].data);
    }
    println!("CS: 0x{:x}  SS: 0x{:x}  DS: 0x{:x}", sregs.cs.selector, sregs.ss.selector, sregs.ds.selector);
    println!("---------------------------------------------------");
    
    // [FIX] 예외 저장 영역 대신 진짜 LPB(0x4000000)를 덤프하여 리스트 연결을 확인합니다.
    println!("Hex Dump of LPB at 0x{:x}:", 0x4000000);
    hex_dump_bytes(vm, 0x4000000, 0x100);
    println!("END------------");

    Ok(())
}

pub fn handle_guest_debug(vm: &mut Vm) -> Result<(), Box<dyn std::error::Error>> {
    let mut regs = vm.vcpu_fd.get_regs()?;
    let start_rip = regs.rip;

    // 1. 가상 주소를 물리 주소로 변환 시도
    let phys_rip = match virt_to_phys(start_rip) {
        Some(p) => p,
        None => {
            // [방어] 변환 실패 시 하위 32비트가 유효한 물리 주소인지 도박 시도 (Identity 매핑 가정)
            let guest_phys_guess = start_rip & 0x1FFF_FFFF; 
            if guest_phys_guess < MEM_SIZE as u64 {
                guest_phys_guess
            } else {
                // 정말 모르겠다면 RIP를 1바이트 강제 전진시켜 무한 루프 탈출
                println!("[DEBUG] RIP 0x{:016x} translation failed. Forcing +1 skip.", start_rip);
                regs.rip += 1;
                vm.vcpu_fd.set_regs(&regs)?;
                return Ok(());
            }
        }
    };

    let opcode = match safe_read_bytes(vm, phys_rip, 2) {
        Ok(bytes) => bytes,
        Err(_) => {
            regs.rip += 1;
            vm.vcpu_fd.set_regs(&regs)?;
            return Ok(());
        }
    };

    // 3. 명령어 스킵 로직
    let mut final_rip = start_rip;

    if opcode[0] == 0xCC {
        // INT 3 (1 byte)
        final_rip += 1;
    } else if opcode[0] == 0xCD && opcode[1] == 0x03 {
        // INT 3 (2 bytes)
        final_rip += 2;
    } else if opcode[0] == 0xCD && opcode[1] == 0x2D {
        // INT 2D (Windows Kernel Debug)
        final_rip += 2;
    } else {
        // [중요] 만약 Debug Exit가 났는데 알려진 INT 3 코드가 아니라면?
        // 싱글 스텝의 결과일 수도 있으므로 강제로 전진하지 않고 리턴합니다.
        return Ok(());
    }

    // CC 패딩 스킵 (Greedy Skip)
    loop {
        let current_phys = match virt_to_phys(final_rip) {
            Some(p) => p,
            None => break,
        };
        match safe_read_bytes(vm, current_phys, 1) {
            Ok(b) if b[0] == 0xCC => final_rip += 1,
            _ => break,
        }
    }

    if final_rip != start_rip {
        regs.rip = final_rip;
        vm.vcpu_fd.set_regs(&regs)?;
    }

    Ok(())
}

pub fn handle_diagnostic_trap(vm: &mut Vm, vector: u8) -> Result<(), Box<dyn std::error::Error>> {
    let save_area_phys = SYSTEM_BASE + 0x60000; 
    let mut regs = vm.vcpu_fd.get_regs()?;
    // 1. 스택에서 예외 프레임(Trap Frame) 읽기
    // Stack Layout: [Error Code?] -> [RIP] -> [CS] -> [RFLAGS] -> [RSP] -> [SS]    
    let rsp_phys = match virt_to_phys(regs.rsp) {
        Some(p) => p,
        None => {
            eprintln!("\n[PANIC] Stack corrupted! RSP: 0x{:016x}", regs.rsp);
            return dump_all_registers(vm);
        }
    };
    // Error Code가 스택에 푸시되는 예외 벡터 목록
    let has_error_code = matches!(vector, 8 | 10 | 11 | 12 | 13 | 14 | 17 | 30);
    let error_code = if has_error_code { safe_read_u64(vm, rsp_phys).unwrap_or(0) } else { 0 };
    let offset = if has_error_code { 8 } else { 0 };
    
    let pushed_rip  = safe_read_u64(vm, rsp_phys + offset).unwrap_or(0);
    // let pushed_cs   = safe_read_u64(vm, rsp_phys + offset + 8).unwrap_or(0);
    let old_rflags  = safe_read_u64(vm, rsp_phys + offset + 16).unwrap_or(0);
    let old_rsp     = safe_read_u64(vm, rsp_phys + offset + 24).unwrap_or(0);

    // 2. IDT 스텁이 백업한 레지스터 복구
    let orig_rax = safe_read_u64(vm, save_area_phys).unwrap_or(0);
    let orig_rcx = safe_read_u64(vm, save_area_phys + 8).unwrap_or(0);
    let orig_rdx = safe_read_u64(vm, save_area_phys + 16).unwrap_or(0);

    // 3. 디버그 예외 처리 (INT 3 or INT 2D via IDT)
    if vector == 3 || vector == 0x2d {
        let mut final_rip = pushed_rip;

        if vector == 0x2d {
            // Windows Debug Service: RAX=0 indicates success to the guest
            regs.rax = 0; 
            regs.rcx = orig_rcx;
            regs.rdx = orig_rdx;
        } else {
            // Breakpoint: Restore original state
            regs.rax = orig_rax;
            regs.rcx = orig_rcx;
            regs.rdx = orig_rdx;
        }

        // Skip current instruction (padding CC)
        loop {
            let p = match virt_to_phys(final_rip) {
                Some(p) => p,
                None => break,
            };
            match safe_read_bytes(vm, p, 1) {
                Ok(b) if b[0] == 0xCC => final_rip += 1,
                _ => break,
            }
        }

        // Resume Guest
        regs.rip = final_rip;
        regs.rsp = old_rsp;
        regs.rflags = old_rflags & !0x100; // Clear Trap Flag
        vm.vcpu_fd.set_regs(&regs)?;
        
        return Ok(());
    }

    // 4. 그 외 예외는 크래시로 간주하고 덤프
    println!("\n[DIAGNOSTIC] Trap triggered! Vector Number: {}", vector);
    if has_error_code {
        println!("Error Code   : 0x{:x}", error_code);
    }
    println!("------------------ EXCEPTION FRAME ------------------");
    println!("FAULTING RIP: 0x{:016x}", pushed_rip);
    println!("STACK DUMP   : 0x{:016x} (Phys: 0x{:x})", regs.rsp, rsp_phys);
    println!("KERNEL RAX   : 0x{:016x}", orig_rax);
    println!("KERNEL RCX   : 0x{:016x}", orig_rcx);
    println!("KERNEL RDX   : 0x{:016x}", orig_rdx);
    
    dump_all_registers(vm)
}


fn virt_to_phys(vaddr: u64) -> Option<u64> {
    // 범위를 128MB(0x8000000)로 대폭 확대하여 더 안전하게 변환합니다.
    
    // 1. Kernel Identity Mapping Area
    let sys_virt_base = 0xFFFFF80000000000 + SYSTEM_BASE;
    if vaddr >= sys_virt_base && vaddr < sys_virt_base + 0x8000000 { 
        return Some((vaddr - sys_virt_base) + SYSTEM_BASE);
    }

    // 2. Kernel Image Area
    if vaddr >= 0xFFFFF80000200000 && vaddr < 0xFFFFF80000200000 + 0x8000000 { 
        return Some((vaddr - 0xFFFFF80000200000) + KRNL_PBASE);
    }

    // 3. HAL Image Area
    if vaddr >= 0x1c0000000 && vaddr < 0x1c0000000 + 0x2000000 { 
        return Some((vaddr - 0x1c0000000) + HAL_PBASE);
    }

    // 4. Kernel Stack Area
    if vaddr >= 0xFFFFFE8000000000 && vaddr < 0xFFFFFE8000000000 + 0x8000000 {
        return Some((vaddr - 0xFFFFFE8000000000) + STACK_PBASE);
    }

    // 5. KUSER_SHARED_DATA Area
    if vaddr >= 0xFFFFF78000000000 && vaddr < 0xFFFFF78000000000 + 0x200000 {
        return Some((vaddr - 0xFFFFF78000000000) + KUSER_PBASE);
    }

    // 6. Identity Map (Boot phase / Low memory)
    if vaddr < MEM_SIZE as u64 {
        return Some(vaddr);
    }

    None // 매핑되지 않은 주소
}

fn safe_read_u64(vm: &Vm, paddr: u64) -> Result<u64, ()> {
    if paddr + 8 > MEM_SIZE as u64 { return Err(()); }
    unsafe {
        let ptr = vm.mem_ptr.add(paddr as usize);
        Ok(u64::from_le_bytes(*(ptr as *const [u8; 8])))
    }
}

fn safe_read_bytes(vm: &Vm, paddr: u64, size: usize) -> Result<Vec<u8>, ()> {
    if paddr + size as u64 > MEM_SIZE as u64 { return Err(()); }
    let mut buf = vec![0u8; size];
    unsafe {
        let src = vm.mem_ptr.add(paddr as usize);
        std::ptr::copy_nonoverlapping(src, buf.as_mut_ptr(), size);
    }
    Ok(buf)
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

pub fn setup_diagnostic_idt(vm: &mut Vm) -> Result<(), Box<dyn std::error::Error>> {
    let idt_pbase = crate::SYSTEM_BASE + 0x20000; 
    let stub_base_p = crate::SYSTEM_BASE + 0x10000; 
    let stub_base_v = 0xFFFFF80000000000 + stub_base_p; 
    let save_area: u64 = crate::SYSTEM_BASE + 0x60000;
    let save_area_v: u64 = 0xFFFFF80000000000 + save_area; 

    for i in 0..256 {
        let mut stub = Vec::new();
        stub.extend_from_slice(&[0x48, 0xA3]); 
        stub.extend_from_slice(&save_area_v.to_le_bytes()); 
        stub.extend_from_slice(&[0x48, 0x89, 0xC8, 0x48, 0xA3]); 
        stub.extend_from_slice(&(save_area_v + 8).to_le_bytes()); 
        stub.extend_from_slice(&[0x48, 0x89, 0xD0, 0x48, 0xA3]); 
        stub.extend_from_slice(&(save_area_v + 16).to_le_bytes()); 
        stub.extend_from_slice(&[0xB0, i as u8, 0xE6, 0xF9, 0xF4]); 
        vm.write_memory((stub_base_p + i as u64 * 64) as usize, &stub).map_err(|e| e.to_string())?;

        let mut entry = [0u8; 16];
        let h = stub_base_v + i as u64 * 64; 
        entry[0..2].copy_from_slice(&(h as u16).to_le_bytes()); 
        entry[2..4].copy_from_slice(&0x10u16.to_le_bytes());    
        entry[5] = 0x8E; 
        entry[6..8].copy_from_slice(&((h >> 16) as u16).to_le_bytes()); 
        entry[8..12].copy_from_slice(&((h >> 32) as u32).to_le_bytes()); 
        vm.write_memory((idt_pbase + i as u64 * 16) as usize, &entry).map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn verify_mapping(_vm: &Vm, _v: u64) {}
