use core::ffi::{c_int, c_uchar, c_void};

// Bindings to the existing xv6 kernel library.
extern "C" {
    // ioapic.c
    pub fn ioapicenable(irq: u32, cpu: u32);

    // console.c
    pub fn cprint(c: *const c_uchar);
    pub fn panic(c: *const c_uchar);

    // kalloc.c
    pub fn kalloc() -> *mut c_void;
    pub fn kfree(ptr: *const c_void);

    // syscall.c
    pub fn argint(n: c_int, ip: *mut c_int);
    pub fn argptr(n: c_int, pp: *const *mut c_void, size: c_int);

    // spinlock.c
    pub fn pushcli();
    pub fn popcli();
}
