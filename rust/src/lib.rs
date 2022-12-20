#![no_std]
#![feature(lang_items)]
#![feature(alloc_error_handler)]

#[macro_use]
extern crate alloc;

use core::panic::PanicInfo;

mod kalloc;
mod kernel;

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

#[no_mangle]
extern "C" fn hello_rust() {
    let lang = "Rust";
    let output = format!("Hello from {lang} in the kernel!\n\x00");
    unsafe {
        kernel::cprint(output.as_ptr());
    }
}
