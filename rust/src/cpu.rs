use core::arch::asm;

/// Approximate CPU frequency in MHz.
pub const CPU_FREQ_MHZ: u64 = 3_000;

/// Read the Time Stamp Counter (TSC) register.
pub fn rdtsc() -> u64 {
    let lo: u32;
    let hi: u32;

    unsafe {
        asm!("rdtsc",  out("edx")hi, out("eax")lo);
    }

    ((hi as u64) << 32) | lo as u64
}
