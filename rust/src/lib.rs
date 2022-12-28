#![no_std]
#![feature(lang_items)]
#![feature(alloc_error_handler)]

extern crate alloc;

use core::panic::PanicInfo;

use crate::kernel::cprint;

mod asm;
mod kalloc;
mod kernel;
mod spinlock;

mod arp;
mod e1000;
mod ethernet;
mod ip;
mod mm;
mod net;
mod pci;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    unsafe {
        cprint("Panic!\n\x00".as_ptr());
    }
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
