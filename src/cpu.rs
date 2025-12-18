// in src/cpu.rs

use crate::vm::Vm;

const MAX_STEPS: usize = 20; // 우선 20번의 명령어만 실행하도록 제한

/// VM을 받아 CPU 에뮬레이션 루프를 실행합니다.
pub fn run(vm: &mut Vm) -> Result<(), &'static str> {
    println!("\n--- CPU Emulation Start ---");

    for step in 0..MAX_STEPS {
        // --- FETCH (명령어 가져오기) ---
        // RIP가 가리키는 주소에서 1바이트를 읽어옵니다. 이것이 명령어 코드(Opcode)입니다.
        let opcode = vm.read_byte(vm.rip)?;

        // --- DECODE & EXECUTE (해석 및 실행) ---
        // 지금은 실제 명령을 해석하는 대신, 현재 상태를 출력만 합니다.
        println!(
            "[Step {:02}] RIP: 0x{:08x}, Fetched Opcode: 0x{:02x}",
            step, vm.rip, opcode
        );

        // --- UPDATE RIP (다음 명령어로 이동) ---
        // 실제 x86 명령어는 길이가 가변적이지만, 지금은 단순하게 1씩만 증가시킵니다.
        vm.rip += 1;
    }

    println!("--- CPU Emulation Finished ({} steps) ---", MAX_STEPS);
    Ok(())
}

