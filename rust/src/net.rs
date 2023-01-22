use alloc::boxed::Box;
use alloc::collections::btree_map::BTreeMap;
use alloc::format;
use alloc::sync::Arc;
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
use crate::packet_buffer::{PacketBuffer, BUFFER_SIZE};
use crate::spinlock::Spinlock;
use crate::udp::UdpPacket;

/// The system network device.
///
/// Assumptions:
/// 	- We only have one network device attached to the system
/// 	- Some driver initialization routine called on system start-up will
///    register the driver with the network stack.
static NETWORK_DEVICE: Spinlock<Option<Box<dyn NetworkDevice>>> = Spinlock::new(None);

/// ARP Cache.
static ARP_CACHE: Spinlock<ArpCache> = Spinlock::new(ArpCache::new());

/// Active system sockets.
static SOCKETS: Spinlock<BTreeMap<usize, Socket>> = Spinlock::new(BTreeMap::new());

/// Represents a device that can send and receive packets.
pub trait NetworkDevice: Send + Sync {
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
    dest_port: Option<u16>,
    dest_protocol_address: Option<Ipv4Addr>,
    dest_hardware_address: Option<EthernetAddress>,
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

    // Setup other buffers and caches.
    let mut sockets = SOCKETS.lock();
    *sockets = BTreeMap::new();

    let mut arp_cache = ARP_CACHE.lock();
    *arp_cache = ArpCache::new();
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

    let socket_id = create_socket(domain);
    socket_id as i32
}

/// The bind system call.
#[no_mangle]
unsafe extern "C" fn sys_bind() {
    cprint("Hello from sys_bind()\n\x00".as_ptr());
}

/// The connect system call.
#[no_mangle]
unsafe extern "C" fn sys_connect() -> i32 {
    let mut socket_id: i32 = 0;
    argint(0, &mut socket_id);

    let mut dest_address: i32 = 0;
    argint(1, &mut dest_address);

    let mut dest_port: i32 = 0;
    argint(2, &mut dest_port);

    match connect(socket_id as u32, dest_address as u32, dest_port as u32) {
        Ok(()) => 0,
        Err(()) => 1,
    }
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
    let data_ptr: *const *mut u8 = &mut data;
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

/// The shutdown system call.
#[no_mangle]
unsafe extern "C" fn sys_shutdown() -> i32 {
    let mut socket_id: i32 = 0;
    argint(0, &mut socket_id);

    cprint("Hello from sys_shutdown()\n\x00".as_ptr());
    match shutdown_socket(socket_id as u32) {
        Ok(_) => 0,
        Err(_) => 1,
    }
}

/// Create a new socket of the specified domain and return the socket identifer.
fn create_socket(domain: SocketType) -> u32 {
    let mut sockets = SOCKETS.lock();
    let socket_id = sockets.len();
    sockets.insert(
        socket_id,
        Socket {
            r#type: domain,
            source_port: None,
            source_address: None,
            dest_port: None,
            dest_protocol_address: None,
            dest_hardware_address: None,
        },
    );
    socket_id as u32
}

/// Associate a socket with an address.
fn connect(socket_id: u32, dest_address: u32, dest_port: u32) -> Result<(), ()> {
    // Look up the desination hardware address from the cache or try and resolve it.
    let dest_protocol_address = Ipv4Addr::from(dest_address as u32);
    let dest_hardware_address = {
        let arp_cache = ARP_CACHE.lock();
        arp_cache.hardware_address(&dest_protocol_address)
    };
    let dest_hardware_address = match dest_hardware_address {
        Some(x) => x,
        None => {
            // Address not in the cache. Make the request, release the device lock try and
            // and block until the address is resolved.
            {
                let mut device = NETWORK_DEVICE.lock();
                let device: &mut Box<dyn NetworkDevice> = match *device {
                    Some(ref mut x) => x,
                    None => return Err(()),
                };
                ArpCache::resolve(&dest_protocol_address, device);
                drop(device);
            }

            // Wait 1 seconds for a response.
            let timeout = rdtsc() + (CPU_FREQ_MHZ * 1_000_000);
            loop {
                if rdtsc() > timeout {
                    break;
                }
            }

            {
                let arp_cache = ARP_CACHE.lock();
                match arp_cache.hardware_address(&dest_protocol_address) {
                    Some(x) => x,
                    None => return Err(()),
                }
            }
        }
    };

    let mut sockets = SOCKETS.lock();
    let mut socket = match sockets.get_mut(&(socket_id as usize)) {
        Some(x) => x,
        None => return Err(()),
    };

    // Populate the Socket with details of the connection.
    socket.dest_protocol_address = Some(dest_protocol_address);
    socket.dest_hardware_address = Some(dest_hardware_address);
    socket.dest_port = Some((dest_port as i16).try_into().unwrap());

    unsafe {
        cprint(
            format!(
                "Hello from sys_connect() {}, {}, {}\n\x00",
                socket_id, dest_address, dest_port
            )
            .as_ptr(),
        );
    }
    Ok(())
}

/// Clean up a socket and its resouces.
fn shutdown_socket(socket_id: u32) -> Result<(), ()> {
    let mut sockets = SOCKETS.lock();
    match sockets.remove(&(socket_id as usize)) {
        Some(_) => Ok(()),
        None => Err(()),
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
            return;
        },
    };

    match ethernet_frame.ethertype {
        Ethertype::IPV4 => {
            let ip_packet = match buffer.parse::<Ipv4Packet>() {
                Ok(x) => x,
                Err(_) => unsafe {
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
                                return;
                            }
                        };
                    }
                    Protocol::TCP => (),
                    Protocol::UNKNOWN => (),
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
            return None;
        },
    };

    match icmp_packet {
        IcmpPacket::EchoMessage(x) => {
            if x.r#type == Type::EchoRequest {
                let reply = IcmpPacket::EchoMessage(IcmpEchoMessage::from_request(x));
                let mut packet = PacketBuffer::new(BUFFER_SIZE);
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
                let mut packet = PacketBuffer::new(BUFFER_SIZE);
                packet.serialize(&reply);
                return Some(packet);
            }
        }
        arp::Operation::Reply => {
            let mut arp_cache = ARP_CACHE.lock();
            arp_cache.reply(arp_packet);
        }
        arp::Operation::Unknown => (),
    }
    None
}
