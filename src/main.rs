mod cpu;
mod loader;
mod pe;
mod vm;

use std::env;
use std::fs;
use std::process;
use vm::{Vm, MEM_SIZE};

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <exe_path>", args[0]);
        process::exit(1);
    }
    let exe_path = &args[1];

    println!("Initializing KVM VM...");
    let mut vm = match Vm::new() {
        Ok(v) => v,
        Err(e) => {
            eprintln!("Failed to create VM: {}", e);
            process::exit(1);
        }
    };

    // 1. 페이지 테이블 설정 (Identity Mapping for 4GB)
    // 64비트 모드에서는 페이지 테이블이 필수입니다.
    // 여기서는 0x0 ~ 0x40000000 (1GB) 영역을 1:1로 매핑합니다.
    println!("Setting up Page Tables...");
    setup_identity_paging(&mut vm).expect("Failed to setup paging");

    // 2. 파일 로드
    println!("Loading file: {}", exe_path);
    let buffer = fs::read(exe_path).expect("Failed to read file");

    let entry_point = if exe_path.ends_with(".bin") {
        println!("Binary file detected. Loading directly at 0x100000...");
        let load_addr = 0x100000;
        vm.write_memory(load_addr, &buffer).expect("Failed to write binary to memory");
        load_addr as u64
    } else {
        let pe_file = pe::parse(&buffer).expect("Failed to parse PE");
        println!("Mapping PE sections to Guest Memory...");
        loader::load_sections(&mut vm, &pe_file).expect("Failed to load PE sections")
    };
    
    println!("File loaded. Entry Point (Phys): 0x{:x}", entry_point);

    // 3. RIP 설정
    let mut regs = vm.vcpu_fd.get_regs().expect("Failed to get regs");
    regs.rip = entry_point;
    regs.rsp = (MEM_SIZE - 0x1000) as u64; // 스택 포인터 설정
    regs.rflags = 0x2; // 기본 플래그
    
    vm.vcpu_fd.set_regs(&regs).expect("Failed to set regs");

    println!("VM initialized. Ready to run.");

    // 4. CPU 실행
    if let Err(e) = cpu::run(&mut vm) {
        eprintln!("CPU Error: {}", e);
        process::exit(1);
    }
}

// 간단한 1:1 페이징 설정 (PML4 -> PDPT -> PD)
// 2MB 페이지를 사용하여 1GB 매핑
fn setup_identity_paging(vm: &mut Vm) -> Result<(), &'static str> {
    // Page Table 위치 (4KB 정렬 필수)
    let pml4_addr: usize = 0x1000;
    let pdpt_addr: usize = 0x2000;
    let pd_addr: usize = 0x3000;

    // 1. 페이지 테이블 영역 초기화 (Zero-out)
    // 쓰레기 값이 있으면 페이지 폴트의 원인이 됨
    vm.write_memory(pml4_addr, &[0u8; 4096])?;
    vm.write_memory(pdpt_addr, &[0u8; 4096])?;
    vm.write_memory(pd_addr, &[0u8; 4096])?;

    // 2. PML4 Entry 0 -> PDPT
    let pml4_entry = (pdpt_addr as u64) | 0x3; // Present | Write
    vm.write_memory(pml4_addr, &pml4_entry.to_le_bytes())?;

    // 3. PDPT Entry 0 -> PD
    let pdpt_entry = (pd_addr as u64) | 0x3; // Present | Write
    vm.write_memory(pdpt_addr, &pdpt_entry.to_le_bytes())?;

    // 4. PD Entries (512개 * 2MB = 1GB)
    for i in 0..512 {
        let page_addr = (i as u64) * 0x200000; // 2MB 단위
        let pd_entry = page_addr | 0x83; // Present | Write | Huge Page (2MB)
        
        let entry_offset = pd_addr + (i * 8);
        vm.write_memory(entry_offset, &pd_entry.to_le_bytes())?;
    }

    Ok(())
}
