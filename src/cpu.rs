use crate::vm::Vm;
use iced_x86::{Code, Decoder, DecoderOptions, Formatter, Instruction, NasmFormatter, OpKind, Register};

const MAX_STEPS: usize = 200; // 20번의 명령어만 실행하도록 임시 제한
const ZERO_FLAG_MASK: u64 = 1 << 6;

/// VM을 받아 CPU 에뮬레이션 루프 실행
pub fn run(vm: &mut Vm) -> Result<(), &'static str> {
    println!("\n--- CPU Emulation Start (with Disassembler) ---");

    // 출력용 포매터
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

        // 5. 명령어 실행 (Execute)
        match instruction.code() {
            Code::Inc_rm64 => {
                match instruction.op0_register() {
                    Register::RAX => vm.rax = vm.rax.wrapping_add(1),
                    Register::RBX => vm.rbx = vm.rbx.wrapping_add(1),
                    Register::RCX => vm.rcx = vm.rcx.wrapping_add(1),
                    Register::RDX => vm.rdx = vm.rdx.wrapping_add(1),
                    Register::RSI => vm.rsi = vm.rsi.wrapping_add(1),
                    Register::RDI => vm.rdi = vm.rdi.wrapping_add(1),
                    Register::RBP => vm.rbp = vm.rbp.wrapping_add(1),
                    Register::RSP => vm.rsp = vm.rsp.wrapping_add(1),
                    Register::R8 => vm.r8 = vm.r8.wrapping_add(1),
                    Register::R9 => vm.r9 = vm.r9.wrapping_add(1),
                    Register::R10 => vm.r10 = vm.r10.wrapping_add(1),
                    Register::R11 => vm.r11 = vm.r11.wrapping_add(1),
                    Register::R12 => vm.r12 = vm.r12.wrapping_add(1),
                    Register::R13 => vm.r13 = vm.r13.wrapping_add(1),
                    Register::R14 => vm.r14 = vm.r14.wrapping_add(1),
                    Register::R15 => vm.r15 = vm.r15.wrapping_add(1),
                    _ => println!("Unsupported INC on register {:?}", instruction.op0_register()),
                }
            }
            // MOV r64, imm64
            Code::Mov_r64_imm64 => {
                let reg = instruction.op0_register();
                let imm = instruction.immediate64();
                match reg {
                    Register::RAX => vm.rax = imm,
                    Register::RBX => vm.rbx = imm,
                    Register::RCX => vm.rcx = imm,
                    Register::RDX => vm.rdx = imm,
                    Register::RSI => vm.rsi = imm,
                    Register::RDI => vm.rdi = imm,
                    Register::RBP => vm.rbp = imm,
                    Register::RSP => vm.rsp = imm,
                    Register::R8 => vm.r8 = imm,
                    Register::R9 => vm.r9 = imm,
                    Register::R10 => vm.r10 = imm,
                    Register::R11 => vm.r11 = imm,
                    Register::R12 => vm.r12 = imm,
                    Register::R13 => vm.r13 = imm,
                    Register::R14 => vm.r14 = imm,
                    Register::R15 => vm.r15 = imm,
                    _ => println!("Unsupported MOV on register {:?}", reg),
                }
            }
            // MOV r32, imm32
            Code::Mov_r32_imm32 => {
                let reg = instruction.op0_register();
                let imm = instruction.immediate32() as u64;
                match reg {
                    Register::EAX => vm.rax = imm,
                    Register::EBX => vm.rbx = imm,
                    Register::ECX => vm.rcx = imm,
                    Register::EDX => vm.rdx = imm,
                    Register::ESI => vm.rsi = imm,
                    Register::EDI => vm.rdi = imm,
                    Register::EBP => vm.rbp = imm,
                    Register::ESP => vm.rsp = imm,
                    Register::R8D => vm.r8 = imm,
                    Register::R9D => vm.r9 = imm,
                    Register::R10D => vm.r10 = imm,
                    Register::R11D => vm.r11 = imm,
                    Register::R12D => vm.r12 = imm,
                    Register::R13D => vm.r13 = imm,
                    Register::R14D => vm.r14 = imm,
                    Register::R15D => vm.r15 = imm,
                    _ => println!("Unsupported MOV on register {:?}", reg),
                }
            }
            // SUB r64, imm8/imm32
            Code::Sub_rm64_imm8 | Code::Sub_rm64_imm32 => {
                let reg = instruction.op0_register();
                let imm = instruction.immediate32() as u64;
                match reg {
                    Register::RAX => vm.rax = vm.rax.wrapping_sub(imm),
                    Register::RBX => vm.rbx = vm.rbx.wrapping_sub(imm),
                    Register::RCX => vm.rcx = vm.rcx.wrapping_sub(imm),
                    Register::RDX => vm.rdx = vm.rdx.wrapping_sub(imm),
                    Register::RSI => vm.rsi = vm.rsi.wrapping_sub(imm),
                    Register::RDI => vm.rdi = vm.rdi.wrapping_sub(imm),
                    Register::RBP => vm.rbp = vm.rbp.wrapping_sub(imm),
                    Register::RSP => vm.rsp = vm.rsp.wrapping_sub(imm),
                    Register::R8 => vm.r8 = vm.r8.wrapping_sub(imm),
                    Register::R9 => vm.r9 = vm.r9.wrapping_sub(imm),
                    Register::R10 => vm.r10 = vm.r10.wrapping_sub(imm),
                    Register::R11 => vm.r11 = vm.r11.wrapping_sub(imm),
                    Register::R12 => vm.r12 = vm.r12.wrapping_sub(imm),
                    Register::R13 => vm.r13 = vm.r13.wrapping_sub(imm),
                    Register::R14 => vm.r14 = vm.r14.wrapping_sub(imm),
                    Register::R15 => vm.r15 = vm.r15.wrapping_sub(imm),
                    _ => println!("Unsupported SUB on register {:?}", reg),
                }
            }
            // ADD r64, imm8/imm32
            Code::Add_rm64_imm8 | Code::Add_rm64_imm32 => {
                let reg = instruction.op0_register();
                let imm = instruction.immediate32() as u64;
                match reg {
                    Register::RAX => vm.rax = vm.rax.wrapping_add(imm),
                    Register::RBX => vm.rbx = vm.rbx.wrapping_add(imm),
                    Register::RCX => vm.rcx = vm.rcx.wrapping_add(imm),
                    Register::RDX => vm.rdx = vm.rdx.wrapping_add(imm),
                    Register::RSI => vm.rsi = vm.rsi.wrapping_add(imm),
                    Register::RDI => vm.rdi = vm.rdi.wrapping_add(imm),
                    Register::RBP => vm.rbp = vm.rbp.wrapping_add(imm),
                    Register::RSP => vm.rsp = vm.rsp.wrapping_add(imm),
                    Register::R8 => vm.r8 = vm.r8.wrapping_add(imm),
                    Register::R9 => vm.r9 = vm.r9.wrapping_add(imm),
                    Register::R10 => vm.r10 = vm.r10.wrapping_add(imm),
                    Register::R11 => vm.r11 = vm.r11.wrapping_add(imm),
                    Register::R12 => vm.r12 = vm.r12.wrapping_add(imm),
                    Register::R13 => vm.r13 = vm.r13.wrapping_add(imm),
                    Register::R14 => vm.r14 = vm.r14.wrapping_add(imm),
                    Register::R15 => vm.r15 = vm.r15.wrapping_add(imm),
                    _ => println!("Unsupported ADD on register {:?}", reg),
                }
            }
            // JMP_rel32_64 (JMP, 상대 주소)
            Code::Jmp_rel32_64 => {
                let target = instruction.near_branch_target();
                println!("    Jumping to 0x{:016x}", target);
                vm.rip = target;
                continue;
            }
            // Jne_rel8_64 (같지 않으면 점프) - RFLAGS를 보고 조건부 점프
            Code::Jne_rel8_64 => {
                // Zero Flag (ZF)가 0인지 확인 (즉, 이전 비교 결과가 같지 않았는지 확인)
                if (vm.rflags & ZERO_FLAG_MASK) == 0 {
                    let target = instruction.near_branch_target();
                    println!("    Conditional JNE: ZF is 0. Jumping to 0x{:016x}", target);
                    vm.rip = target;
                    continue;
                } else {
                    println!("    Conditional JNE: ZF is 1. Not jumping.");
                }
            }
            // LEA r64, m (주소 계산 후 레지스터에 저장)
            Code::Lea_r64_m => {
                let dst_reg = instruction.op0_register();
                let effective_address = calculate_memory_address(vm, &instruction);
                set_register_value(vm, dst_reg, effective_address);
                println!("    LEA {:?} <- 0x{:016x}", dst_reg, effective_address);
            }
            // CALL_rel32_64 (CALL, 상대 주소)
            Code::Call_rel32_64 => {
                let return_address = vm.rip + instruction_len as u64;
                vm.rsp = vm.rsp.wrapping_sub(8); // 스택 공간 확보
                vm.write_qword(vm.rsp, return_address)?; // 스택에 돌아올 주소 저장
                let target = instruction.near_branch_target();
                println!("    Calling 0x{:016x}, return to 0x{:016x}", target, return_address);
                vm.rip = target;
                continue;
            }
            // RETnq (RET, 함수에서 돌아옴)
            Code::Retnq => {
                let return_address = vm.read_qword(vm.rsp)?; // 스택에서 돌아올 주소 읽기
                vm.rsp = vm.rsp.wrapping_add(8); // 스택 공간 정리
                println!("    Returning to 0x{:016x}", return_address);
                vm.rip = return_address;
                continue;
            }
            Code::Call_rm64 => { // 레지스터나 메모리 참조를 통한 CALL - 일단 Unhandled
                println!("    --> Unhandled Instruction: Call_rm64. Complex call type, skipping for now.");
            }
            // PUSH r64 (예: push rbp)
            Code::Push_r64 => {
                let reg = instruction.op0_register();
                let value_to_push = get_register_value(vm, reg);
                vm.rsp = vm.rsp.wrapping_sub(8); // 스택 공간 확보
                vm.write_qword(vm.rsp, value_to_push)?;
                println!("    PUSH {:?} (0x{:016x}) -> RSP: 0x{:016x}", reg, value_to_push, vm.rsp);
            }
            // POP r64 (예: pop rbp)
            Code::Pop_r64 => {
                let reg = instruction.op0_register();
                let popped_value = vm.read_qword(vm.rsp)?; // 스택에서 값 읽기
                vm.rsp = vm.rsp.wrapping_add(8); // 스택 공간 정리
                set_register_value(vm, reg, popped_value);
                println!("    POP {:?} (0x{:016x}) -> RSP: 0x{:016x}", reg, popped_value, vm.rsp);
            }
            // CMP r64, rm64 (예: cmp rax, rbx)
            Code::Cmp_r64_rm64 => {
                let reg1 = instruction.op0_register();
                let val1 = get_register_value(vm, reg1);

                let src_op_kind = instruction.op1_kind();
                let val2 = match src_op_kind {
                    OpKind::Register => {
                        let reg2 = instruction.op1_register();
                        get_register_value(vm, reg2)
                    }
                    _ => {
                        println!("    --> Unhandled CMP: Source is not a register. Skipping.");
                        0
                    }
                };

                let result = val1.wrapping_sub(val2);

                // Zero Flag (ZF) 설정
                if result == 0 {
                    vm.rflags |= ZERO_FLAG_MASK; // 결과가 0이면 ZF를 1로 설정
                } else {
                    vm.rflags &= !ZERO_FLAG_MASK; // 결과가 0이 아니면 ZF를 0으로 설정
                }
                println!("    CMP {:?}, {:?} -> result: {}, ZF set to {}", reg1, instruction.op1_register(), result, (vm.rflags & ZERO_FLAG_MASK) != 0);
            }
            // MOV r64, rm64
            Code::Mov_r64_rm64 => {
                let dst_reg = instruction.op0_register();
                let src_value = match instruction.op1_kind() {
                    OpKind::Register => get_register_value(vm, instruction.op1_register()),
                    OpKind::Memory => {
                        let addr = calculate_memory_address(vm, &instruction);
                        vm.read_qword(addr)?
                    }
                    _ => {
                        println!("    --> Unhandled MOV_r64_rm64: Unexpected source operand kind.");
                        0
                    }
                };
                set_register_value(vm, dst_reg, src_value);
                println!("    MOV {:?} <- 0x{:016x}", dst_reg, src_value);
            }
            // MOV rm64, r64
            Code::Mov_rm64_r64 => {
                if instruction.op0_kind() == OpKind::Memory {
                    let addr = calculate_memory_address(vm, &instruction);
                    let src_reg = instruction.op1_register();
                    let src_value = get_register_value(vm, src_reg);
                    vm.write_qword(addr, src_value)?;
                    println!("    MOV [0x{:016x}] <- {:?} (0x{:016x})", addr, src_reg, src_value);
                } else {
                    println!("    --> Unhandled MOV_rm64_r64: Destination is not memory.");
                }
            }
            _ => {
                println!("    --> Unhandled Instruction: {:?}", instruction.code());
            }
        }

        println!(
            "[Step {:02}] RIP: 0x{:08x} (len: {}) -> {}",
            step, vm.rip, instruction_len, output
        );
        println!(
            "                  RAX: 0x{:016x} RCX: 0x{:016x} RDX: 0x{:016x}\n                  RSP: 0x{:016x} RFLAGS: 0x{:016x}",
            vm.rax, vm.rcx, vm.rdx, vm.rsp, vm.rflags
        );

        // 6. RIP를 실제 명령어 길이만큼 증가시켜 다음 명령어를 가리키게 함
        vm.rip += instruction_len as u64;
    }

    println!("--- CPU Emulation Finished ({} steps) ---", MAX_STEPS);
    Ok(())
}

/// 레지스터 번호(iced_x86::Register)에 해당하는 VM 레지스터의 값을 가져옴
fn get_register_value(vm: &Vm, reg: Register) -> u64 {
    match reg {
        Register::RAX => vm.rax,
        Register::RBX => vm.rbx,
        Register::RCX => vm.rcx,
        Register::RDX => vm.rdx,
        Register::RSI => vm.rsi,
        Register::RDI => vm.rdi,
        Register::RBP => vm.rbp,
        Register::RSP => vm.rsp,
        Register::R8 => vm.r8,
        Register::R9 => vm.r9,
        Register::R10 => vm.r10,
        Register::R11 => vm.r11,
        Register::R12 => vm.r12,
        Register::R13 => vm.r13,
        Register::R14 => vm.r14,
        Register::R15 => vm.r15,
        // 32비트 레지스터(EAX 등)가 들어오면 해당 64비트 레지스터의 하위 32비트만 반환
        // 지금은 편의상 64비트 값을 반환하되, 나중에 확장될 수 있음
        Register::EAX => vm.rax & 0xFFFFFFFF,
        Register::ECX => vm.rcx & 0xFFFFFFFF,
        Register::EDX => vm.rdx & 0xFFFFFFFF,
        Register::EBX => vm.rbx & 0xFFFFFFFF,
        Register::ESP => vm.rsp & 0xFFFFFFFF,
        Register::EBP => vm.rbp & 0xFFFFFFFF,
        Register::ESI => vm.rsi & 0xFFFFFFFF,
        Register::EDI => vm.rdi & 0xFFFFFFFF,
        Register::R8D => vm.r8 & 0xFFFFFFFF,
        Register::R9D => vm.r9 & 0xFFFFFFFF,
        Register::R10D => vm.r10 & 0xFFFFFFFF,
        Register::R11D => vm.r11 & 0xFFFFFFFF,
        Register::R12D => vm.r12 & 0xFFFFFFFF,
        Register::R13D => vm.r13 & 0xFFFFFFFF,
        Register::R14D => vm.r14 & 0xFFFFFFFF,
        Register::R15D => vm.r15 & 0xFFFFFFFF,
        // 기타 레지스터는 일단 0 반환
        _ => {
            println!("Warning: Attempted to get value of unhandled register {:?}", reg);
            0
        }
    }
}

/// 레지스터 번호(iced_x86::Register)에 해당하는 VM 레지스터에 값을 설정
fn set_register_value(vm: &mut Vm, reg: Register, value: u64) {
    match reg {
        Register::RAX => vm.rax = value,
        Register::RBX => vm.rbx = value,
        Register::RCX => vm.rcx = value,
        Register::RDX => vm.rdx = value,
        Register::RSI => vm.rsi = value,
        Register::RDI => vm.rdi = value,
        Register::RBP => vm.rbp = value,
        Register::RSP => vm.rsp = value,
        Register::R8 => vm.r8 = value,
        Register::R9 => vm.r9 = value,
        Register::R10 => vm.r10 = value,
        Register::R11 => vm.r11 = value,
        Register::R12 => vm.r12 = value,
        Register::R13 => vm.r13 = value,
        Register::R14 => vm.r14 = value,
        Register::R15 => vm.r15 = value,
        // 32비트 레지스터에 쓸 때는 상위 32비트 0 처리 (자동 확장)
        Register::EAX => vm.rax = value & 0xFFFFFFFF,
        Register::ECX => vm.rcx = value & 0xFFFFFFFF,
        Register::EDX => vm.rdx = value & 0xFFFFFFFF,
        Register::EBX => vm.rbx = value & 0xFFFFFFFF,
        Register::ESP => vm.rsp = value & 0xFFFFFFFF,
        Register::EBP => vm.rbp = value & 0xFFFFFFFF,
        Register::ESI => vm.rsi = value & 0xFFFFFFFF,
        Register::EDI => vm.rdi = value & 0xFFFFFFFF,
        Register::R8D => vm.r8 = value & 0xFFFFFFFF,
        Register::R9D => vm.r9 = value & 0xFFFFFFFF,
        Register::R10D => vm.r10 = value & 0xFFFFFFFF,
        Register::R11D => vm.r11 = value & 0xFFFFFFFF,
        Register::R12D => vm.r12 = value & 0xFFFFFFFF,
        Register::R13D => vm.r13 = value & 0xFFFFFFFF,
        Register::R14D => vm.r14 = value & 0xFFFFFFFF,
        Register::R15D => vm.r15 = value & 0xFFFFFFFF,
        _ => println!("Warning: Attempted to set value of unhandled register {:?}", reg),
    }
}

/// 명령어의 메모리 피연산자를 기반으로 실제 메모리 주소를 계산
fn calculate_memory_address(vm: &Vm, instruction: &Instruction) -> u64 {
    if instruction.is_ip_rel_memory_operand() {
        // RIP 상대 주소 계산 (예: mov rax, [rel 1234h])
        instruction.ip_rel_memory_address()
    } else {
        // 일반적인 Base + (Index * Scale) + Displacement 주소 계산
        let base_reg = instruction.memory_base();
        let index_reg = instruction.memory_index();
        let displ = instruction.memory_displacement64();
        let scale = instruction.memory_index_scale() as u64;

        let mut addr: u64 = 0;

        if base_reg != Register::None {
            addr = addr.wrapping_add(get_register_value(vm, base_reg));
        }
        if index_reg != Register::None {
            let index_val = get_register_value(vm, index_reg);
            addr = addr.wrapping_add(index_val.wrapping_mul(scale));
        }
        addr.wrapping_add(displ)
    }
}

