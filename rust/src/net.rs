use alloc::format;
use core::slice;

use crate::ethernet::EthernetHeader;
use crate::kernel::cprint;

/// Main entrypoint into the kernel network stack.
pub unsafe fn handle_packet(data: &[u8]) {
    let ethernet_header = EthernetHeader::from_slice(data);
    cprint(format!("{:x?}\n\x00", ethernet_header).as_ptr());
}

