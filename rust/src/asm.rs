use core::arch::asm;

/// TODO: Wrap I/O port methods in a nicer interface.

/// Output a double word to the port specified by `port`.
pub unsafe fn out_dw(port: u16, data: u32) {
    asm!("out dx, eax", in("dx")port, in("eax")data);
}

/// Read a double word to the port specified by `port`.
pub unsafe fn in_dw(port: u16) -> u32 {
    let mut result: u32;
    asm!("in eax, dx", in("dx")port, out("eax")result);
    result
}
