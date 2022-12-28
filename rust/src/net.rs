use alloc::format;
use core::slice;

use crate::arp;
use crate::e1000;
use crate::ethernet::{EthernetAddress, EthernetHeader, Ethertype};
use crate::ip::Ipv4Addr;
use crate::kernel::cprint;

/// We have a single, static IP address (10.0.0.2) for now.
static IpAddress: [u8; 4] = [0x0A, 0x00, 0x00, 0x02];

/// The context associated with a packet.
pub struct PacketContext {
    /// The hardware address associated with the interface that received the packet.
    pub mac_address: EthernetAddress,
}

/// Main entrypoint into the kernel network stack.
pub unsafe fn handle_packet(data: &[u8], ctx: &PacketContext) {
    let ethernet_header = EthernetHeader::from_slice(data);

    cprint(format!("{:x?}\n\x00", ethernet_header).as_ptr());
    match ethernet_header.ethertype {
        Ethertype::IPV4 => cprint("IPV4 Packet\n\x00".as_ptr()),
        Ethertype::ARP => handle_arp(&data[14..], &ctx),
        Ethertype::WAKE_ON_LAN => cprint("WAKE_ON_LAN Packet\n\x00".as_ptr()),
        Ethertype::RARP => cprint("RARP Packet\n\x00".as_ptr()),
        Ethertype::SLPP => cprint("SLPP Packet\n\x00".as_ptr()),
        Ethertype::IPV6 => cprint("IPV6 Packet\n\x00".as_ptr()),
        Ethertype::UNKNOWN => cprint("Unknown Packet\n\x00".as_ptr()),
    }
}

/// Handle an ARP packet.
pub unsafe fn handle_arp(data: &[u8], ctx: &PacketContext) {
    let arp_packet = arp::Packet::from_slice(&data);

    match arp_packet.oper {
        arp::Operation::Request => {
            // Is this a request for us?
            if arp_packet.tpa == Ipv4Addr::from_slice(&IpAddress) {
                cprint(format!("ARP request for us ({:?})\n\x00", arp_packet.tpa).as_ptr());
                // Build a new reply.
                cprint("Building reply from request\n\x00".as_ptr());
                let reply = arp::Packet::from_request(&arp_packet, ctx.mac_address);
                cprint("Built reply from request\n\x00".as_ptr());
            }
        }
        arp::Operation::Reply => {
            cprint("ARP Response\n\x00".as_ptr());
        }
        arp::Operation::Unknown => (),
    }
}
