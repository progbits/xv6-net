use alloc::format;
use alloc::vec;
use alloc::vec::Vec;
use core::slice;

use crate::arp;
use crate::e1000;
use crate::ethernet::{EthernetAddress, EthernetFrame, Ethertype};
use crate::ip::Ipv4Addr;
use crate::kernel::cprint;

/// We have a single, static IP address (10.0.0.2) for now.
static IpAddress: [u8; 4] = [0x0A, 0x00, 0x00, 0x02];

/// The context associated with a packet.
pub struct PacketContext {
    /// The hardware address associated with the interface that received the packet.
    pub mac_address: EthernetAddress,
}

/// Represents a type that can be parsed from a PacketBuffer.
pub trait FromBuffer {
    /// Parse a new instance from a slice of bytes.
    fn from_buffer(buf: &[u8]) -> Self;

    /// The size of the parsed structure, not including any encapsulated data.
    fn size(&self) -> usize;
}

/// Represents raw packet data.
///
/// TODO: Stack allocated buffer?
pub struct PacketBuffer {
    /// The raw packet data.
    buf: Vec<u8>,
    /// The size of the raw packet.
    size: usize,
    /// The number of bytes we have parsed so far into the buffer.
    offset: usize,
}

impl PacketBuffer {
    pub fn new(data: *const u8, size: usize) -> PacketBuffer {
        let mut packet_buffer = PacketBuffer {
            buf: vec![0u8; size],
            size: size,
            offset: 0,
        };
        unsafe {
            core::ptr::copy(data, packet_buffer.buf.as_mut_ptr(), size);
        }
        packet_buffer
    }

    /// Parse a new packet from the buffer.
    /// TODO: Zero-copy?
    pub fn parse<T: FromBuffer>(&mut self) -> T {
        let value = T::from_buffer(&self.buf[self.offset..]);
        self.offset += value.size();
        value
    }
}

/// Main entrypoint into the kernel network stack.
///
/// TODO: Remove `unsafe`.
pub unsafe fn handle_packet(mut buffer: PacketBuffer, ctx: &PacketContext) {
    let ethernet_frame = buffer.parse::<EthernetFrame>();

    cprint(format!("{:x?}\n\x00", ethernet_frame).as_ptr());
    match ethernet_frame.ethertype {
        Ethertype::IPV4 => cprint("IPV4 Packet\n\x00".as_ptr()),
        Ethertype::ARP => handle_arp(&mut buffer, &ctx),
        Ethertype::WAKE_ON_LAN => cprint("WAKE_ON_LAN Packet\n\x00".as_ptr()),
        Ethertype::RARP => cprint("RARP Packet\n\x00".as_ptr()),
        Ethertype::SLPP => cprint("SLPP Packet\n\x00".as_ptr()),
        Ethertype::IPV6 => cprint("IPV6 Packet\n\x00".as_ptr()),
        Ethertype::UNKNOWN => cprint("Unknown Packet\n\x00".as_ptr()),
    }
}

/// Handle an ARP packet.
pub fn handle_arp(buffer: &mut PacketBuffer, ctx: &PacketContext) {
    let arp_packet = buffer.parse::<arp::Packet>();

    match arp_packet.oper {
        arp::Operation::Request => {
            // Is this a request for us?
            if arp_packet.tpa == Ipv4Addr::from_slice(&IpAddress) {
                let _ = arp::Packet::from_request(&arp_packet, ctx.mac_address);
            }
        }
        arp::Operation::Reply => {}
        arp::Operation::Unknown => (),
    }
}
