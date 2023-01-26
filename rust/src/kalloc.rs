use crate::kernel::{cprint, kalloc, kfree};
use crate::mm::PAGE_SIZE;

use core::alloc::{GlobalAlloc, Layout};
use core::ffi::c_void;

struct KernelAllocator {}

#[alloc_error_handler]
fn alloc_error(_: core::alloc::Layout) -> ! {
    loop {}
}

#[global_allocator]
static ALLOCATOR: KernelAllocator = KernelAllocator {};

unsafe impl Sync for KernelAllocator {}

unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.size() > PAGE_SIZE {
            panic!("alloc larger than page\n\x00")
        }
        let mem = kalloc() as *mut u8;
        if mem.is_null() {
            panic!("alloc failed\n\x00")
        }
        mem
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        kfree(ptr as *const c_void)
    }
}
