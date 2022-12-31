use alloc::boxed::Box;
use alloc::format;

use alloc::vec;
use alloc::vec::Vec;


use crate::arp;
use crate::e1000::E1000;
use crate::ethernet::{EthernetAddress, EthernetFrame, Ethertype};
use crate::ip::{Ipv4Addr};
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
    /// Has the buffer been written to?
    written: bool,
}

impl PacketBuffer {
    pub fn new(size: usize) -> PacketBuffer {
        PacketBuffer {
            buf: vec![0u8; size],
            size: size,
            offset: 0,
            written: false,
        }
    }

    pub fn new_from_bytes(data: *const u8, size: usize) -> PacketBuffer {
        let mut packet_buffer = PacketBuffer {
            buf: vec![0u8; size],
            size: size,
            offset: 0,
            written: false,
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

    /// Serialize a new packet to the buffer.
    /// TODO: Zero-copy?
    pub fn serialize<T: ToBuffer>(&mut self, value: &T) {
        self.offset += value.size();
        self.written = true;
        let start = self.buf.len() - self.offset;
        let end = start + value.size();
        unsafe {
            cprint(format!("writing to buffer {}, {}\n\x00", start, end).as_ptr());
        }
        value.to_buffer(&mut self.buf[start..end]);
    }

    pub fn len(&self) -> usize {
        self.offset
    }

    pub fn as_bytes(&self) -> &[u8] {
        if self.written {
            unsafe {
                cprint(format!("offset {}\n\x00", self.offset).as_ptr());
            }
            &self.buf[self.buf.len() - self.offset..]
        } else {
            self.buf.as_slice()
        }
    }
}

/// Represents a type that can be parsed from a PacketBuffer.
pub trait FromBuffer {
    /// Parse a new instance from a slice of bytes.
    fn from_buffer(buf: &[u8]) -> Self;

    /// The size of the parsed structure, not including any encapsulated data.
    fn size(&self) -> usize;
}

/// Represents a type that can be serialized to a PacketBuffer.
pub trait ToBuffer {
    /// Parse a new instance from a slice of bytes.
    fn to_buffer(&self, buf: &mut [u8]);

    /// The size of the serialized structure, including any encapsulated data.
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

    // Handle all avaliable packets.
    loop {
        match device.recv() {
            Some(b) => handle_packet(b, &mut device),
            None => break,
        }
    }
}

/// Main entrypoint into the kernel network stack.
///
/// Handles a single, ethernet frame encapsulated packet. Potentially writes packets back to the network device.
///
/// TODO: A better abstraction for serializing packets.
pub fn handle_packet(mut buffer: PacketBuffer, device: &mut Box<dyn NetworkDevice>) {
    let ethernet_frame = buffer.parse::<EthernetFrame>();
    unsafe {
        cprint(format!("{:x?}\n\x00", ethernet_frame).as_ptr());
    }

    match ethernet_frame.ethertype {
        Ethertype::IPV4 => (),
        Ethertype::ARP => match handle_arp(&mut buffer, &device) {
            Some(mut x) => {
                // Encapsulate the ARP response.
                let ethernet_frame =
                    EthernetFrame::new(ethernet_frame.source, device.mac_address(), Ethertype::ARP);
                x.serialize(&ethernet_frame);
                device.send(x);
            }
            None => (),
        },
        Ethertype::WAKE_ON_LAN => (),
        Ethertype::RARP => (),
        Ethertype::SLPP => (),
        Ethertype::IPV6 => (),
        Ethertype::UNKNOWN => (),
    }
}

/// Handle an ARP packet.
///
/// Handle an ARP packet, optionally returning any response that needs to be serialized to the network.
pub fn handle_arp(
    buffer: &mut PacketBuffer,
    device: &Box<dyn NetworkDevice>,
) -> Option<PacketBuffer> {
    let arp_packet = buffer.parse::<arp::Packet>();

    match arp_packet.oper {
        arp::Operation::Request => {
            // Is this a request for us?
            if arp_packet.tpa == Ipv4Addr::from_slice(&IpAddress) {
                // Build the ARP reply.
                let mac = device.mac_address();
                let reply = arp::Packet::from_request(&arp_packet, mac);
                let mut packet = PacketBuffer::new(1024);
                packet.serialize(&reply);
                return Some(packet);
            }
        }
        arp::Operation::Reply => {}
        arp::Operation::Unknown => (),
    }
    None
}
