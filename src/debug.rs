// src/debug.rs
use crate::vm::Vm;
use kvm_bindings::Msrs;
use kvm_bindings::kvm_msr_entry;
use crate::SYSTEM_BASE;

/// 모든 레지스터 상태를 출력합니다.
pub fn dump_all_registers(vm: &mut Vm) -> Result<(), Box<dyn std::error::Error>> {
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

/// 0xF9 포트로 들어온 예외 번호를 분석하고 상태를 출력합니다.
pub fn handle_diagnostic_trap(vm: &mut Vm, vector: u8) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n\n[DIAGNOSTIC] Trap triggered! Vector Number: {}", vector);
    
    let regs = vm.vcpu_fd.get_regs()?;
    
    // 블랙박스(0x70000)에서 저장된 레지스터 읽기
    let (real_rax, real_rcx, real_rdx) = unsafe {
        let ptr = vm.mem_ptr.add(0x70000) as *const u64;
        (*ptr, *ptr.add(1), *ptr.add(2))
    };

    let has_error = match vector { 8 | 10 | 11 | 12 | 13 | 14 | 17 | 21 => true, _ => false };
    let rsp_p = if regs.rsp >= 0xFFFFF80000000000 { regs.rsp - 0xFFFFF80000000000 } else { regs.rsp };

    println!("------------------ CPU STATE ------------------");
    if rsp_p < vm.mem_size as u64 - 40 {
        unsafe {
            let stack_ptr = vm.mem_ptr.add(rsp_p as usize) as *const u64;
            let fault_rip = if has_error { *stack_ptr.add(1) } else { *stack_ptr };
            println!("FAULTING RIP: 0x{:x}", fault_rip);
            if has_error { println!("Error Code (from stack): 0x{:x}", *stack_ptr); }
        }
    }
    println!("KERNEL RAX: 0x{:016x}", real_rax);
    println!("KERNEL RCX: 0x{:016x}", real_rcx);
    println!("KERNEL RDX: 0x{:016x}", real_rdx);
    
    dump_all_registers(vm)?;
    Ok(())
}

/// 진단용 IDT를 설치합니다. (모든 예외를 0xF9 포트로 전송)
pub fn setup_diagnostic_idt(vm: &mut Vm) -> Result<(), &'static str> {
    let stub_base = SYSTEM_BASE + 0x10000;
    let save_area: u64 = 0x70000; 

    for i in 0..256 {
        let mut stub = Vec::new();
        // RAX, RCX, RDX 보존
        stub.extend_from_slice(&[0x48, 0xA3]); stub.extend_from_slice(&save_area.to_le_bytes());
        stub.extend_from_slice(&[0x48, 0x89, 0xC8, 0x48, 0xA3]); stub.extend_from_slice(&(save_area + 8).to_le_bytes());
        stub.extend_from_slice(&[0x48, 0x89, 0xD0, 0x48, 0xA3]); stub.extend_from_slice(&(save_area + 16).to_le_bytes());
        // 벡터 전송 및 HLT
        stub.extend_from_slice(&[0xB0, i as u8, 0xE6, 0xF9, 0xF4]); 
        
        vm.write_memory((stub_base + i as u64 * 64) as usize, &stub)?;

        let mut entry = [0u8; 16];
        let h = stub_base + i as u64 * 64;
        entry[0..2].copy_from_slice(&(h as u16).to_le_bytes());
        entry[2..4].copy_from_slice(&0x10u16.to_le_bytes()); 
        entry[5] = 0x8E;
        entry[6..8].copy_from_slice(&((h >> 16) as u16).to_le_bytes());
        entry[8..12].copy_from_slice(&((h >> 32) as u32).to_le_bytes());
        vm.write_memory(i * 16, &entry)?;
    }
    Ok(())
}

/// 특정 가상 주소의 매핑 여부를 검증합니다.
pub fn verify_mapping(vm: &Vm, virt_addr: u64) {
    let pml4_base = SYSTEM_BASE as usize + 0x1000;
    let pml4_idx = (virt_addr >> 39) & 0x1FF;
    unsafe {
        let pml4_e = *(vm.mem_ptr.add(pml4_base + (pml4_idx as usize * 8)) as *const u64);
        if pml4_e & 1 == 0 { 
            println!("  [VERIFY] PML4[{}] FAILED (0x{:x})", pml4_idx, virt_addr); 
            return; 
        }
        println!("  [VERIFY] 0x{:x} -> Mapping Found", virt_addr);
    }
}
