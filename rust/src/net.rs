use alloc::boxed::Box;
use alloc::format;
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
use core::slice;

use crate::arp;
use crate::e1000::E1000;
use crate::ethernet::{EthernetAddress, EthernetFrame, Ethertype};
use crate::ip::{Ipv4Addr, Ipv4Packet};
use crate::kernel::cprint;
use crate::spinlock::Spinlock;

/// We have a single, static IP address (10.0.0.2) for now.
static IpAddress: [u8; 4] = [0x0A, 0x00, 0x00, 0x02];

/// Current assumptions:
///		- We only have one network device attached to the system
///		- Some driver initialization routine called on system start-up will register the driver with the network stack.
pub static NETWORK_DEVICE: Spinlock<Option<Box<dyn NetworkDevice>>> = Spinlock::new(None);

/// Represents a device that can send and receive packets.
pub trait NetworkDevice {
    /// The hardware address of the device.
    fn mac_address(&self) -> EthernetAddress;

    /// Clear interrupts.
    fn clear_interrupts(&mut self);

    /// Serialize a new packet.
    fn send(&mut self, buf: PacketBuffer);

    /// Receive a new packet.
    fn recv(&mut self) -> Option<PacketBuffer>;
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

    pub fn len(&self) -> usize {
        self.buf.len()
    }

    pub fn as_bytes(&self) -> &[u8] {
        self.buf.as_slice()
    }
}

/// Represents a type that can be parsed from a PacketBuffer.
pub trait FromBuffer {
    /// Parse a new instance from a slice of bytes.
    fn from_buffer(buf: &[u8]) -> Self;

    /// The size of the parsed structure, not including any encapsulated data.
    fn size(&self) -> usize;
}

/// Initialize the network stack.
///
/// Called on system start-up to initialize the kernel network stack. Routine searches for a compatible E1000 family network device and registers that device in the `NETWORK_DEVICE` global.
#[no_mangle]
unsafe extern "C" fn rustnetinit() {
    cprint("Configuring E1000 family device.\n\x00".as_ptr());
    let e1000_device = match E1000::new() {
        Some(x) => x,
        None => panic!(),
    };
    let mut device = NETWORK_DEVICE.lock();
    *device = Some(Box::new(e1000_device));
    cprint("Done configuring E1000 family device.\n\x00".as_ptr());
}

/// Entrypoint for network device interrupts.
#[no_mangle]
unsafe extern "C" fn netintr() {
    let mut device = NETWORK_DEVICE.lock();
    let mut device: &mut Box<dyn NetworkDevice> = match *device {
        Some(ref mut x) => x,
        None => panic!(),
    };
    // Clear device interrupt register.
    device.clear_interrupts();

    // Handle avaliable packets.
    loop {
        match device.recv() {
            Some(b) => handle_packet(b, &device),
            None => break,
        }
    }
}

/// Main entrypoint into the kernel network stack.
pub fn handle_packet(mut buffer: PacketBuffer, device: &Box<dyn NetworkDevice>) {
    let ethernet_frame = buffer.parse::<EthernetFrame>();

    unsafe {
        cprint(format!("{:x?}\n\x00", ethernet_frame).as_ptr());
    }
    match ethernet_frame.ethertype {
        Ethertype::IPV4 => {
            let ip_packet = buffer.parse::<Ipv4Packet>();
            unsafe {
                cprint(format!("{:x?}\n\x00", ethernet_frame).as_ptr());
            }
        }
        Ethertype::ARP => handle_arp(&mut buffer, &device),
        Ethertype::WAKE_ON_LAN => (),
        Ethertype::RARP => (),
        Ethertype::SLPP => (),
        Ethertype::IPV6 => (),
        Ethertype::UNKNOWN => (),
    }
}

/// Handle an ARP packet.
pub fn handle_arp(buffer: &mut PacketBuffer, device: &Box<dyn NetworkDevice>) {
    let arp_packet = buffer.parse::<arp::Packet>();

    unsafe {
        cprint(format!("{:x?}\n\x00", arp_packet).as_ptr());
    }

    match arp_packet.oper {
        arp::Operation::Request => {
            // Is this a request for us?
            if arp_packet.tpa == Ipv4Addr::from_slice(&IpAddress) {
                let mac = device.mac_address();
                let _ = arp::Packet::from_request(&arp_packet, mac);
            }
        }
        arp::Operation::Reply => {}
        arp::Operation::Unknown => (),
    }
}
