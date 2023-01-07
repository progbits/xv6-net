///
/// Memory management.
///
/// The facilities provided here are currently fairly minimal until we have
/// all of the memory management code written in Rust.

pub const PAGE_SIZE: usize = 1 << 12;

// These constants currently have to match the values in memlayout.h
const EXTMEM: u64 = 0x100000; // Start of extended memory.
const PHYSTOP: u64 = 0xE000000; // Top physical memory.
const DEVSPACE: u64 = 0xFE000000; // Other devices are at high addresses.
const KERNBASE: u64 = 0x80000000; // First kernel virtual address.
const KERNLINK: u64 = KERNBASE + EXTMEM; // Address where kernel is linked.

/// A physical memory addess.
#[derive(Debug, Default)]
#[repr(C)]
pub struct PhysicalAddress(pub u64);

impl PhysicalAddress {
    /// TODO: Sanity checks.
    pub fn new(addr: u64) -> Self {
        PhysicalAddress(addr)
    }

    pub fn from_virtual(addr: u64) -> Self {
        PhysicalAddress(addr - KERNBASE)
    }

    pub fn to_virtual(&self) -> VirtualAddress {
        VirtualAddress(self.0 + KERNBASE)
    }
}

/// A virtual memory addess.
#[derive(Debug, Default)]
#[repr(C)]
pub struct VirtualAddress(pub u64);

impl VirtualAddress {
    /// TODO: Sanity checks.
    pub fn new(addr: u64) -> Self {
        VirtualAddress(addr)
    }

    pub fn from_physical(addr: u64) -> Self {
        VirtualAddress(addr + KERNBASE)
    }

    pub fn to_physical(&self) -> PhysicalAddress {
        PhysicalAddress(self.0 - KERNBASE)
    }
}
