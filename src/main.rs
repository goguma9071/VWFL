use std::env;
use std::fs;
use std::process;

mod pe;
mod vm;
mod loader;
mod cpu; // cpu 모듈 선언

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <path-to-pe-file>", args[0]);
        process::exit(1);
    }

    let file_path = &args[1];
    let buffer = match fs::read(file_path) {
        Ok(buf) => buf,
        Err(e) => {
            eprintln!("Failed to read file '{}': {}", file_path, e);
            process::exit(1);
        }
    };

    println!("Parsing file: {}", file_path);

    match pe::PeFile::from_bytes(&buffer) {
        Ok(pe_file) => {
            println!("..Ok. File parsed successfully.");
            println!("{}", pe_file);
            
            // 1. 가상 머신 생성
            const MEM_SIZE: usize = 1024 * 1024 * 256;
            let mut vm = vm::Vm::new(MEM_SIZE);

            // 2. 섹션들을 메모리에 적재
            println!("\nLoading sections into memory...");
            if let Err(e) = loader::load_sections(&mut vm, &pe_file) {
                eprintln!("Error loading sections: {}", e);
                std::process::exit(1);
            }
            println!("Done.");

            // 3. 재배치된 주소로 RIP 설정
            vm.rip = pe_file.entry_point - pe_file.image_base;
            println!("VM RIP set to rebased Entry Point: 0x{:x}", vm.rip);

            // 4. CPU 에뮬레이션 시작
            if let Err(e) = cpu::run(&mut vm) {
                eprintln!("CPU Error: {}", e);
                process::exit(1);
            }

            // --- Serial Port Test ---
            println!("\n--- Serial Port Test Start ---");
            vm.write_byte(vm::SERIAL_PORT_ADDRESS, 'H' as u8).unwrap();
            vm.write_byte(vm::SERIAL_PORT_ADDRESS, 'e' as u8).unwrap();
            vm.write_byte(vm::SERIAL_PORT_ADDRESS, 'l' as u8).unwrap();
            vm.write_byte(vm::SERIAL_PORT_ADDRESS, 'l' as u8).unwrap();
            vm.write_byte(vm::SERIAL_PORT_ADDRESS, 'o' as u8).unwrap();
            println!("\n--- Serial Port Test Finished ---");
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            process::exit(1);
        }
    }
}