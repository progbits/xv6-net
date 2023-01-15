use alloc::boxed::Box;
use alloc::collections::btree_map::BTreeMap;
use alloc::format;
use alloc::vec;
use alloc::vec::Vec;
use core::ffi::c_void;

use crate::arp;
use crate::arp::{ArpCache, ArpPacket};
use crate::cpu::{rdtsc, CPU_FREQ_MHZ};
use crate::e1000::E1000;
use crate::ethernet::{EthernetAddress, EthernetFrame, Ethertype};
use crate::icmp::IcmpPacket;
use crate::icmp::{IcmpEchoMessage, Type};
use crate::ip::{Ipv4Addr, Ipv4Packet, Protocol};
use crate::kernel::{argint, argptr, cprint};
use crate::spinlock::Spinlock;
use crate::udp::UdpPacket;

pub static PACKET_BUFFER_SIZE: usize = 2048;

/// Current assumptions:
/// 		- We only have one network device attached to the system
/// 		- Some driver initialization routine called on system start-up will
///     register the driver with the network stack.
pub static NETWORK_DEVICE: Spinlock<Option<Box<dyn NetworkDevice>>> = Spinlock::new(None);

/// Active system sockets.
static SOCKETS: Spinlock<BTreeMap<usize, Socket>> = Spinlock::new(BTreeMap::new());

#[derive(Debug)]
enum SocketType {
    TCP,
    UDP,
}

/// Represents one end of a socket connection.
#[derive(Debug)]
struct Socket {
    r#type: SocketType,
    source_port: Option<u16>,
    source_address: Option<Ipv4Addr>,
    destination_port: Option<u16>,
    destination_protocol_address: Option<Ipv4Addr>,
    destination_hardware_address: Option<EthernetAddress>,
}

/// Represents a device that can send and receive packets.
pub trait NetworkDevice {
    /// The hardware (MAC) address of the device.
    fn hardware_address(&self) -> EthernetAddress;

    /// The protocol (IP) address of the device.
    fn protocol_address(&self) -> Ipv4Addr;

    /// Set the protocol address of the device.
    fn set_protocol_address(&mut self, protocol_address: Ipv4Addr);

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
    /// Create a new buffer with the specified size.
    pub fn new(size: usize) -> PacketBuffer {
        PacketBuffer {
            buf: vec![0u8; size],
            size: size,
            offset: 0,
            written: false,
        }
    }

    /// Create a new buffer from the data provided.
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
    pub fn parse<T: FromBuffer>(&mut self) -> Result<T, ()> {
        let value = match T::from_buffer(&self.buf[self.offset..]) {
            Ok(x) => x,
            Err(_) => return Err(()),
        };
        self.offset += value.size();
        Ok(value)
    }

    /// Serialize a new packet to the buffer.
    /// TODO: Zero-copy?
    pub fn serialize<T: ToBuffer>(&mut self, value: &T) {
        self.offset += value.size();
        self.written = true;
        let start = self.buf.len() - self.offset;
        let end = start + value.size();
        value.to_buffer(&mut self.buf[start..end]);
    }

    /// Return the size of the buffer.
    pub fn len(&self) -> usize {
        self.offset
    }

    /// Return a pointer to the underlying buffer.
    pub fn as_ptr(&self) -> *const u8 {
        if self.written {
            self.buf[self.buf.len() - self.offset..].as_ptr()
        } else {
            self.buf[..self.offset].as_ptr()
        }
    }
}

/// Represents a type that can be parsed from a PacketBuffer.
pub trait FromBuffer {
    /// Parse a new instance from a slice of bytes.
    fn from_buffer(buf: &[u8]) -> Result<Self, ()>
    where
        Self: Sized;

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
/// Called on system start-up to initialize the kernel network stack. Routine
/// searches for a compatible E1000 family network device and registers that
/// device in the `NETWORK_DEVICE` global.
#[no_mangle]
unsafe extern "C" fn rustnetinit() {
    // Setup the network device and panic if no device is avaliable.
    let e1000_device = match E1000::new() {
        Some(x) => x,
        None => panic!(),
    };

    // Assign a hardcoded, static IP to the device for now.
    let mut network_device = NETWORK_DEVICE.lock();
    let mut device = Box::new(e1000_device);
    device.set_protocol_address(Ipv4Addr::from(0x0A000002 as u32));
    *network_device = Some(device);
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
/// Handles a single, ethernet frame encapsulated packet. Potentially writes
/// packets back to the network device.
pub fn handle_packet(mut buffer: PacketBuffer, device: &mut Box<dyn NetworkDevice>) {
    let ethernet_frame = match buffer.parse::<EthernetFrame>() {
        Ok(x) => x,
        Err(_) => unsafe {
            cprint("Error parsing ethernet frame\n\x00".as_ptr());
            return;
        },
    };

    match ethernet_frame.ethertype {
        Ethertype::IPV4 => {
            let ip_packet = match buffer.parse::<Ipv4Packet>() {
                Ok(x) => x,
                Err(_) => unsafe {
                    cprint("Unexpected IPV4 header size\n\x00".as_ptr());
                    return;
                },
            };

            unsafe {
                match ip_packet.protocol() {
                    Protocol::ICMP => match handle_icmp(&mut buffer, &device) {
                        Some(mut x) => {
                            let ip_packet = Ipv4Packet::new(
                                0,
                                0,
                                (x.len() + 20) as u16,
                                0,
                                true,
                                false,
                                0,
                                64,
                                Protocol::ICMP,
                                device.protocol_address(),
                                ip_packet.source(),
                            );
                            x.serialize(&ip_packet);

                            let ethernet_frame = EthernetFrame::new(
                                ethernet_frame.source,
                                device.hardware_address(),
                                Ethertype::IPV4,
                            );
                            x.serialize(&ethernet_frame);
                            device.send(x);
                        }
                        None => (),
                    },
                    Protocol::UDP => {
                        let udp_packet = match buffer.parse::<UdpPacket>() {
                            Ok(x) => x,
                            Err(_) => {
                                cprint("Error parsing udp packet\n\x00".as_ptr());
                                return;
                            }
                        };
                    }
                    Protocol::TCP => cprint("TCP/IP packet\n\x00".as_ptr()),
                    Protocol::UNKNOWN => cprint("UNKNOWN/IP Packet\n\x00".as_ptr()),
                }
            }
        }
        Ethertype::ARP => match handle_arp(&mut buffer, &device) {
            Some(mut x) => {
                // Encapsulate the ARP response.
                let ethernet_frame = EthernetFrame::new(
                    ethernet_frame.source,
                    device.hardware_address(),
                    Ethertype::ARP,
                );
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

/// Handle an ICMP packet.
pub fn handle_icmp(
    buffer: &mut PacketBuffer,
    device: &Box<dyn NetworkDevice>,
) -> Option<PacketBuffer> {
    let icmp_packet = match buffer.parse::<IcmpPacket>() {
        Ok(x) => x,
        Err(_) => unsafe {
            cprint("Error parsing ICMP packet\n\x00".as_ptr());
            return None;
        },
    };

    match icmp_packet {
        IcmpPacket::EchoMessage(x) => {
            if x.r#type == Type::EchoRequest {
                let reply = IcmpPacket::EchoMessage(IcmpEchoMessage::from_request(x));
                let mut packet = PacketBuffer::new(PACKET_BUFFER_SIZE);
                packet.serialize(&reply);
                return Some(packet);
            }
        }
        _ => panic!(),
    }
    None
}

/// Handle an ARP packet.
///
/// Handle an ARP packet, optionally returning any response that needs to be
/// serialized to the network.
pub fn handle_arp(
    buffer: &mut PacketBuffer,
    device: &Box<dyn NetworkDevice>,
) -> Option<PacketBuffer> {
    let arp_packet = match buffer.parse::<ArpPacket>() {
        Ok(x) => x,
        Err(_) => unsafe {
            cprint("Error parsing ARP packet\n\x00".as_ptr());
            return None;
        },
    };

    // Get the protocol address of the device.
    match arp_packet.oper {
        arp::Operation::Request => {
            // Is this a request for us?
            if arp_packet.tpa == device.protocol_address() {
                // Build the ARP reply.
                let hardware_address = device.hardware_address();
                let reply = ArpPacket::from_request(&arp_packet, hardware_address);
                let mut packet = PacketBuffer::new(PACKET_BUFFER_SIZE);
                packet.serialize(&reply);
                return Some(packet);
            }
        }
        arp::Operation::Reply => ArpCache::reply(arp_packet),
        arp::Operation::Unknown => (),
    }
    None
}

/// The socket system call.
///
/// Creates a new Socket entry and returns the socket identifier. Note,
/// descriptors returned by this method are not currently compatable with the
/// generalized read(...) and write(...) system calls.
#[no_mangle]
unsafe extern "C" fn sys_socket() -> i32 {
    let mut domain: i32 = 0;
    argint(0, &mut domain);

    let domain = match domain {
        0 => SocketType::UDP,
        _ => return -1,
    };

    let mut sockets = SOCKETS.lock();
    let socket_id = sockets.len();
    sockets.insert(
        socket_id,
        Socket {
            r#type: domain,
            source_port: None,
            source_address: None,
            destination_port: None,
            destination_protocol_address: None,
            destination_hardware_address: None,
        },
    );

    return socket_id as i32;
}

/// The bind system call.
#[no_mangle]
unsafe extern "C" fn sys_bind() {
    cprint("Hello from sys_bind()\n\x00".as_ptr());
}

/// The connect system call.
///
/// TODO:
/// 	- Sanitise arguments.
/// 	- Don't hold locks for so long.
#[no_mangle]
unsafe extern "C" fn sys_connect() -> i32 {
    let mut socket_id: i32 = 0;
    argint(0, &mut socket_id);

    let mut destination_address: i32 = 0;
    argint(1, &mut destination_address);

    let mut destination_port: i32 = 0;
    argint(2, &mut destination_port);

    let mut sockets = SOCKETS.lock();
    let mut socket = match sockets.get_mut(&(socket_id as usize)) {
        Some(x) => x,
        None => return -1,
    };

    // Populate the Socket with details of the connection.
    let destination_protocol_address = Ipv4Addr::from(destination_address as u32);
    socket.destination_protocol_address = Some(destination_protocol_address);
    socket.destination_port = Some((destination_port as i16).try_into().unwrap());

    // Look up the desination hardware address from the cache or try and resolve it.
    let destination_hardware_address = ArpCache::hardware_address(&destination_protocol_address);
    match destination_hardware_address {
        Some(x) => x,
        None => {
            // Address not in the cache. Make the request, release the device lock try and
            // and block until the address is resolved.
            {
                let mut device = NETWORK_DEVICE.lock();
                let device: &mut Box<dyn NetworkDevice> = match *device {
                    Some(ref mut x) => x,
                    None => return -1,
                };
                ArpCache::resolve(&destination_protocol_address, device);
                drop(device);
            }

            // Wait 2 seconds for a response.
            let timeout = rdtsc() + (CPU_FREQ_MHZ * 2_000_000);
            loop {
                if rdtsc() > timeout {
                    break;
                }
            }

            match ArpCache::hardware_address(&destination_protocol_address) {
                Some(x) => x,
                None => return -1,
            }
        }
    };

    cprint(
        format!(
            "Hello from sys_connect() {}, {}, {}\n\x00",
            socket_id, destination_address, destination_port
        )
        .as_ptr(),
    );

    return 0;
}

/// The listen system call.
#[no_mangle]
unsafe extern "C" fn sys_listen() {
    cprint("Hello from sys_listen()\n\x00".as_ptr());
}

/// The accept system call.
#[no_mangle]
unsafe extern "C" fn sys_accept() {
    cprint("Hello from sys_accept()\n\x00".as_ptr());
}

/// The send system call.
#[no_mangle]
unsafe extern "C" fn sys_send() {
    let mut socket_id: i32 = 0;
    argint(0, &mut socket_id);

    let mut data: *mut u8 = core::ptr::null_mut();
    let mut data_ptr: *const *mut u8 = &mut data;
    argptr(1, data_ptr as _, 4);

    let mut len: i32 = 0;
    argint(2, &mut len);

    cprint(
        format!(
            "Hello from sys_send() {}, {}, {}\n\x00",
            socket_id, *data, len
        )
        .as_ptr(),
    );
}

/// The recv system call.
#[no_mangle]
unsafe extern "C" fn sys_recv() {
    cprint("Hello from sys_recv()\n\x00".as_ptr());
}
/// The accept system call.
#[no_mangle]
unsafe extern "C" fn sys_shutdown() {
    cprint("Hello from sys_shutdown()\n\x00".as_ptr());
}
