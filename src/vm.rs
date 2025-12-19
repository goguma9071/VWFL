use std::io::{self, Write};

pub const SERIAL_PORT_ADDRESS: u64 = 0x10000000;

#[derive(Debug)]
pub struct Vm {
//... (이 부분은 이전과 동일)
    pub memory: Vec<u8>,
    pub rip: u64,
    // General-purpose registers
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub rsp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
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