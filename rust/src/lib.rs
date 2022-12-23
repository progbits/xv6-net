#![no_std]
#![feature(lang_items)]
#![feature(alloc_error_handler)]

extern crate alloc;

use core::panic::PanicInfo;

mod asm;
mod kalloc;
mod kernel;
mod spinlock;

mod e1000;
mod mm;
mod pci;

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
#[allow(non_snake_case)]
pub fn _Unwind_Resume() {
    loop {}
}

#[lang = "eh_personality"]
#[no_mangle]
pub extern "C" fn eh_personality() {}
