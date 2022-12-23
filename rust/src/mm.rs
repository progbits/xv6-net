///
/// Memory management.
///
/// The facilities provided here are currently fairly minimal until we have
/// all of the memory management code written in Rust.
///

pub const PAGE_SIZE: usize = 1 << 12;

// These constants currently have to match the values in memlayout.h
const EXTMEM: u32 = 0x100000; // Start of extended memory.
const PHYSTOP: u32 = 0xE000000; // Top physical memory.
const DEVSPACE: u32 = 0xFE000000; // Other devices are at high addresses.
const KERNBASE: u32 = 0x80000000; // First kernel virtual address.
const KERNLINK: u32 = KERNBASE + EXTMEM; // Address where kernel is linked.

/// A physical memory addess.
#[derive(Debug, Default)]
pub struct PhysicalAddress(u32);

impl PhysicalAddress {
    /// TODO: Sanity checks.
    pub fn new(addr: u32) -> PhysicalAddress {
        PhysicalAddress(addr)
    }

    pub fn to_virtual_address(&self) -> VirtualAddress {
        VirtualAddress(self.0 + KERNBASE)
    }

    pub fn value(&self) -> u32 {
        self.0
    }
}

/// A virtual memory addess.
#[derive(Debug, Default)]
pub struct VirtualAddress(u32);

impl VirtualAddress {
    /// TODO: Sanity checks.
    pub fn new(addr: u32) -> VirtualAddress {
        VirtualAddress(addr)
    }

    pub fn to_physical_address(&self) -> PhysicalAddress {
        PhysicalAddress(self.0 - KERNBASE)
    }

    pub fn value(&self) -> u32 {
        self.0
    }
}
