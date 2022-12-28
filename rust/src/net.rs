use alloc::format;
use core::slice;

use crate::arp;
use crate::ethernet::{EthernetHeader, Ethertype};
use crate::kernel::cprint;

/// Main entrypoint into the kernel network stack.
pub unsafe fn handle_packet(data: &[u8]) {
    let ethernet_header = EthernetHeader::from_slice(data);

    cprint(format!("{:x?}\n\x00", ethernet_header).as_ptr());
    match ethernet_header.ethertype {
        Ethertype::IPV4 => cprint("IPV4 Packet\n\x00".as_ptr()),
        Ethertype::ARP => handle_arp(&data[14..]),
        Ethertype::WAKE_ON_LAN => cprint("WAKE_ON_LAN Packet\n\x00".as_ptr()),
        Ethertype::RARP => cprint("RARP Packet\n\x00".as_ptr()),
        Ethertype::SLPP => cprint("SLPP Packet\n\x00".as_ptr()),
        Ethertype::IPV6 => cprint("IPV6 Packet\n\x00".as_ptr()),
        Ethertype::UNKNOWN => cprint("Unknown Packet\n\x00".as_ptr()),
    }
}

/// Handle an ARP packet.
pub unsafe fn handle_arp(data: &[u8]) {
    let arp_packet = arp::Packet::from_slice(&data);

    match arp_packet.oper {
        arp::Operation::Request => {
            cprint(format!("ARP Request for {:?}\n\x00", arp_packet.tpa).as_ptr());
        }
        arp::Operation::Reply => {
            cprint("ARP Response\n\x00".as_ptr());
        }
        arp::Operation::Unknown => (),
    }
}
