use crate::vm::Vm;
use iced_x86::{Decoder, DecoderOptions, Formatter, NasmFormatter};

const MAX_STEPS: usize = 20; // 우선 20번의 명령어만 실행하도록 제한

/// VM을 받아 CPU 에뮬레이션 루프를 실행합니다.
pub fn run(vm: &mut Vm) -> Result<(), &'static str> {
    println!("\n--- CPU Emulation Start (with Disassembler) ---");

    // Nasm 문법으로 명령어를 이쁘게 출력해줄 포매터입니다.
    let mut formatter = NasmFormatter::new();

    for step in 0..MAX_STEPS {
        // 1. 루프 안에서 매번 현재의 vm.rip 값을 기반으로 디코더를 새로 생성

        // 1a. vm.rip (u64)를 메모리 인덱싱을 위한 usize로 변환
        let rip_as_usize: usize = match vm.rip.try_into() {
            Ok(addr) => addr,
            Err(_) => return Err("RIP value is too large to fit into usize on this platform."),
        };

        // 1b. 현재 rip 위치부터 메모리 끝까지의 코드 조각을 가져옴
        let code_slice = &vm.memory[rip_as_usize..];

        // 1c. 이 코드 조각으로 새로운 디코더를 생성하고, 디코더에게 현재 rip(u64) 값 통보
        let mut decoder = Decoder::new(64, code_slice, DecoderOptions::NONE);
        decoder.set_ip(vm.rip);

        let instruction = decoder.decode();

        // 3. 디코딩된 명령어의 길이 가져오기
        let instruction_len = instruction.len();
        if instruction_len == 0 {
            return Err("Failed to decode instruction or reached end of code");
        }

        // 4. (출력용) 디코딩된 명령어를 문자열로 변환
        let mut output = String::new();
        formatter.format(&instruction, &mut output);

        // 5. 현재 상태 출력
        println!(
            "[Step {:02}] RIP: 0x{:08x} (len: {}) -> {}",
            step, vm.rip, instruction_len, output
        );

        // 6. RIP를 실제 명령어 길이만큼 증가시켜 다음 명령어를 가리키게 함
        vm.rip += instruction_len as u64;
    }

    println!("--- CPU Emulation Finished ({} steps) ---", MAX_STEPS);
    Ok(())
}
