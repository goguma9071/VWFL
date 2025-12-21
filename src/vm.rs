use std::io::{self, Write};

pub const SERIAL_PORT_ADDRESS: u64 = 0x10000000;

#[derive(Debug)]
pub struct Vm {

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
    pub rflags: u64,
}

impl Vm {

    pub fn new(memory_size: usize) -> Self {
        Vm {
            memory: vec![0; memory_size],
            rip: 0,
            rax: 0,
            rbx: 0,
            rcx: 0,
            rdx: 0,
            rsi: 0,
            rdi: 0,
            rbp: 0,
            rsp: 0,
            r8: 0,
            r9: 0,
            r10: 0,
            r11: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
            rflags: 0,
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

    /// 지정된 주소에서 8바이트(qword)를 읽어 u64로 반환
    pub fn read_qword(&self, address: u64) -> Result<u64, &'static str> {
        let addr = address as usize;
        if addr + 8 > self.memory.len() {
            return Err("Memory access out of bounds (read_qword)");
        }
        let bytes: [u8; 8] = self.memory[addr..addr + 8].try_into().unwrap();
        Ok(u64::from_le_bytes(bytes))
    }

    /// 지정된 주소에 u64 값을 8바이트(qword)로 씀
    pub fn write_qword(&mut self, address: u64, value: u64) -> Result<(), &'static str> {
        let addr = address as usize;
        if addr + 8 > self.memory.len() {
            return Err("Memory access out of bounds (write_qword)");
        }
        self.memory[addr..addr + 8].copy_from_slice(&value.to_le_bytes());
        Ok(())
    }

    /// 스택에 값을 넣음 (RSP 감소 후 쓰기)
    pub fn push(&mut self, value: u64) -> Result<(), &'static str> {
        self.rsp = self.rsp.wrapping_sub(8);
        self.write_qword(self.rsp, value)
    }

    /// 스택에서 값을 꺼냄 (읽은 후 RSP 증가)
    pub fn pop(&mut self) -> Result<u64, &'static str> {
        let value = self.read_qword(self.rsp)?;
        self.rsp = self.rsp.wrapping_add(8);
        Ok(value)
    }
}