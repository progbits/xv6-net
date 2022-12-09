#![no_std]
#![feature(c_variadic)]

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    unsafe {
        printf(1, "panic!".as_ptr());
    }
    loop {}
}

extern "C" {
    // printf.c
    pub fn printf(fd: i32, c: *const u8, args: ...);
}

#[no_mangle]
fn hello_world() {
    unsafe {
        printf(1, "hello from Rust!\n".as_bytes().as_ptr());
    }
}
