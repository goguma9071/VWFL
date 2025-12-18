use std::io::{self, Write};

pub const SERIAL_PORT_ADDRESS: u64 = 0x10000000;

#[derive(Debug)]
pub struct Vm {
//... (이 부분은 이전과 동일)
    memory: Vec<u8>,
    pub rip: u64,    // Instruction Pointer (명령어 포인터)
    // 나중에 스택 포인터 (rsp), 일반 목적 레지스터 등도 추가될 수 있습니다.
}

impl Vm {
//... (new, read_byte 함수는 이전과 동일)
    pub fn new(memory_size: usize) -> Self {
        Vm {
            memory: vec![0; memory_size],
            rip: 0,
        }
    }

    pub fn read_byte(&self, address: u64) -> Result<u8, &'static str> {
        if address as usize >= self.memory.len() {
            return Err("Address out of range (read)");
        }
        Ok(self.memory[address as usize])
    }

    pub fn write_byte(&mut self, address: u64, value: u8) -> Result<(), &'static str> {
        // MMIO: Check if the address is our magic serial port address
        if address == SERIAL_PORT_ADDRESS {
            // Print the character to the console
            print!("{}", value as char);
            // Flush stdout to ensure the character appears immediately
            io::stdout().flush().unwrap();
            return Ok(());
        }

        if address as usize >= self.memory.len() {
            return Err("Address out of range (write)");
        }
        self.memory[address as usize] = value;
        Ok(())
    }
}