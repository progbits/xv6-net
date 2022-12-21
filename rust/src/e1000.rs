use crate::spinlock::Spinlock;

use crate::kernel::cprint;

/// A representation of the e1000 family device state.
struct E1000 {
    mmio_base: u32,
    //mac_addr: [u8; 6],
    //rx: u8,
    //rx_buf: u8,
    //rx_count: u8,
    //rx_idx: u8,
    //tx: u8,
    //tx_buf: u8,
    //tx_count: u8,
    //tx_idx: u8,
    //tx_ctx: u8,
}

static E1000: Spinlock<E1000> = Spinlock::<E1000>::new(E1000 { mmio_base: 0x0 });

#[no_mangle]
extern "C" fn e1000_init() {
    let mut e1000 = E1000.lock();
    e1000.mmio_base = 10;

    unsafe {
        cprint(b"Hello from e1000_init\n\x00".as_ptr());
    }
}
