#![no_std]
#![no_main]
extern crate alloc;

mod loaderblock;
mod nt_types;
mod redsea;

use core::panic::PanicInfo;
use core::arch::asm;

pub fn out(s: &str) {
    for b in s.bytes() {
        unsafe {
            asm!(
            "out, dx, dl",
            in("dx") 0x3F8u16, // DX = 0x3f8 (COM1 Port)
                in("al") b
            );
        }
    }
    unsafe {
        asm!("hlt")
    }
}

#[no_mangle]
pub extern "C" fn _start() -> ! {
    loop {}
}



struct d{}