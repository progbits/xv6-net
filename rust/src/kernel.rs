use core::ffi::{c_uchar, c_void};

// Bindings to the existing xv6 kernel library.
extern "C" {
    // ioapic.c
    pub fn ioapicenable(irq: u32, cpu: u32);

    // console.c
    pub fn cprint(c: *const c_uchar);

    // kalloc.c
    pub fn kalloc() -> *mut c_void;
    pub fn kfree(ptr: *const c_void);
}
