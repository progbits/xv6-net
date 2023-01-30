use alloc::boxed::Box;
use alloc::collections::btree_map::BTreeMap;
use alloc::vec;
use alloc::vec::Vec;
use core::slice;

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
    _TCP,
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
    buffer: Vec<u8>,
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
        None => panic!("no network device\n\x00"),
    };
    cprint("Configured E1000 family device\n\x00".as_ptr());

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
    handle_interrupt();
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
unsafe extern "C" fn sys_bind() -> i32 {
    let mut socket_id: i32 = 0;
    argint(0, &mut socket_id);

    let mut source_address: i32 = 0;
    argint(1, &mut source_address);

    let mut source_port: i32 = 0;
    argint(2, &mut source_port);

    let source_port: u16 = match source_port.try_into() {
        Ok(x) => x,
        Err(_) => return 1,
    };

    match bind(socket_id as u32, source_address as u32, source_port) {
        Ok(()) => 0,
        Err(()) => 1,
    }
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

    if socket_id < 0 {
        return 1;
    }

    match connect(socket_id as u32, dest_address as u32, dest_port as u32) {
        Ok(()) => 0,
        Err(()) => 1,
    }
}

/// The listen system call.
#[no_mangle]
unsafe extern "C" fn sys_listen() {}

/// The accept system call.
#[no_mangle]
unsafe extern "C" fn sys_accept() {}

/// The send system call.
#[no_mangle]
unsafe extern "C" fn sys_send() -> i32 {
    let mut socket_id: i32 = 0;
    argint(0, &mut socket_id);

    let mut data: *mut u8 = core::ptr::null_mut();
    let data_ptr: *const *mut u8 = &mut data;
    argptr(1, data_ptr as _, 4);

    let mut len: i32 = 0;
    argint(2, &mut len);
    let len = len as usize;

    let data = unsafe { slice::from_raw_parts(data, len) };

    match send(socket_id as u32, &data) {
        Ok(n) => match i32::try_from(n) {
            Ok(n) => n,
            Err(_) => {
                // i32::max() is much (much) larger than any current hardware supported frame
                // size, so this should never happen under normal operation.
                panic!("send\n\x00")
            }
        },
        Err(_) => -1,
    }
}

/// The recv system call.
#[no_mangle]
unsafe extern "C" fn sys_recv() -> i32 {
    let mut socket_id: i32 = 0;
    argint(0, &mut socket_id);

    let mut data: *mut u8 = core::ptr::null_mut();
    let data_ptr: *const *mut u8 = &mut data;
    argptr(1, data_ptr as _, 4);

    let mut len: i32 = 0;
    argint(2, &mut len);
    let len = len as usize;

    let mut data = slice::from_raw_parts_mut(data, len);

    match recv(socket_id as u32, &mut data, len as u32) {
        Ok(n) => n as i32,
        Err(_) => return -1,
    }
}

/// The shutdown system call.
#[no_mangle]
unsafe extern "C" fn sys_shutdown() -> i32 {
    let mut socket_id: i32 = 0;
    argint(0, &mut socket_id);

    match shutdown_socket(socket_id as u32) {
        Ok(_) => 0,
        Err(_) => 1,
    }
}

/// Create a new socket of the specified domain and return the socket identifer.
fn create_socket(domain: SocketType) -> u32 {
    let mut sockets = SOCKETS.lock();
    let socket_id = sockets.len();
    let mut buffer = vec::Vec::<u8>::new();
    buffer.reserve(BUFFER_SIZE);
    sockets.insert(
        socket_id,
        Socket {
            r#type: domain,
            source_port: None,
            source_address: None,
            dest_port: None,
            dest_protocol_address: None,
            dest_hardware_address: None,
            buffer: buffer,
        },
    );
    socket_id as u32
}

/// Bind a socket to a local address and port.
///
/// TODO:
/// 	- Don't hardcode address to 10.0.0.2
fn bind(socket_id: u32, _source_address: u32, source_port: u16) -> Result<(), ()> {
    let mut sockets = SOCKETS.lock();
    let mut socket = match sockets.get_mut(&(socket_id as usize)) {
        Some(x) => x,
        None => return Err(()),
    };

    socket.source_port = Some(source_port);
    socket.source_address = Some(Ipv4Addr::from(0x0A000002 as u32));

    Ok(())
}

/// Connect to a remote socket.
fn connect(socket_id: u32, dest_address: u32, dest_port: u32) -> Result<(), ()> {
    let mut sockets = SOCKETS.lock();
    let mut socket = match sockets.get_mut(&(socket_id as usize)) {
        Some(x) => x,
        None => return Err(()),
    };

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

    // Populate the Socket with the address of the local adaptor, a new ephermal
    // port and the details of the remote.
    socket.source_port = Some((1024 + socket_id) as u16);
    socket.source_address = Some(Ipv4Addr::from(0x0A000002 as u32));
    socket.dest_port = Some((dest_port as i16).try_into().unwrap());
    socket.dest_hardware_address = Some(dest_hardware_address);
    socket.dest_protocol_address = Some(dest_protocol_address);

    Ok(())
}

/// Encapsulate and send data on a socket.
///
/// TODO:
/// 	- Refactor packet building to one place.
/// 	- Check socket has been setup with connect(...).
fn send(socket_id: u32, data: &[u8]) -> Result<u32, ()> {
    let mut sockets = SOCKETS.lock();
    let socket = match sockets.get_mut(&(socket_id as usize)) {
        Some(x) => x,
        None => return Err(()),
    };

    // Create a new packet buffer.
    let mut packet = PacketBuffer::new(BUFFER_SIZE);

    // Build and write the udp packet, sending up to a maximum of 1024 bytes.
    let data_len: u16 = if data.len() > 1024 {
        1024
    } else {
        data.len() as u16
    };

    let udp_packet = UdpPacket::new(
        socket.source_port.unwrap(),
        socket.dest_port.unwrap(),
        data[..data_len as usize].to_vec(),
    );
    packet.serialize(&udp_packet);

    // Build and write the IP packet.
    let ip_packet = Ipv4Packet::new(
        0,
        0,
        (packet.len() + 20) as u16,
        0,
        true,
        false,
        0,
        64,
        Protocol::UDP,
        socket.source_address.unwrap(),
        socket.dest_protocol_address.unwrap(),
    );
    packet.serialize(&ip_packet);

    // Write the ethernet frame header and send the frame.
    let mut device = NETWORK_DEVICE.lock();
    let device: &mut Box<dyn NetworkDevice> = match *device {
        Some(ref mut x) => x,
        None => return Err(()),
    };

    let ethernet_frame = EthernetFrame::new(
        socket.dest_hardware_address.unwrap(),
        device.hardware_address(),
        Ethertype::IPV4,
    );
    packet.serialize(&ethernet_frame);

    device.send(packet);

    // Encapsulate the data in a UDP packet.
    Ok(data_len as u32)
}

/// Read available data from a socket.
///
/// This call is non-blocking, returning immediately if no data is
/// available.
fn recv(socket_id: u32, data: &mut [u8], len: u32) -> Result<u32, ()> {
    let mut sockets = SOCKETS.lock();
    let socket = match sockets.get_mut(&(socket_id as usize)) {
        Some(x) => x,
        None => return Err(()),
    };

    // Does the socket have any data available?
    if socket.buffer.len() == 0 {
        return Ok(0);
    }

    // Copy the most data we can from the socket buffer to userspace buffer.
    let len = len as usize;
    let copy_size = if socket.buffer.len() > len {
        len
    } else {
        socket.buffer.len()
    };
    data[..copy_size].copy_from_slice(&socket.buffer[..copy_size]);

    // Shrink the socket buffer to remove the copied data.
    let new_size = socket.buffer.len() - copy_size;
    socket.buffer.truncate(new_size);

    Ok(copy_size.try_into().unwrap())
}

/// Clean up a socket and its resouces.
fn shutdown_socket(socket_id: u32) -> Result<(), ()> {
    let mut sockets = SOCKETS.lock();
    match sockets.remove(&(socket_id as usize)) {
        Some(_) => Ok(()),
        None => Err(()),
    }
}

/// Main entrypoint for network device interrupts.
fn handle_interrupt() {
    let mut device = NETWORK_DEVICE.lock();
    let mut device: &mut Box<dyn NetworkDevice> = match *device {
        Some(ref mut x) => x,
        None => panic!("no network device\n\x00"),
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
fn handle_packet(mut buffer: PacketBuffer, device: &mut Box<dyn NetworkDevice>) {
    let ethernet_frame = match buffer.parse::<EthernetFrame>() {
        Ok(x) => x,
        Err(_) => return,
    };

    match ethernet_frame.ethertype {
        Ethertype::IPV4 => {
            let ip_packet = match buffer.parse::<Ipv4Packet>() {
                Ok(x) => x,
                Err(_) => return,
            };

            match ip_packet.protocol() {
                Protocol::ICMP => match handle_icmp(&mut buffer) {
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
                    handle_udp(&mut buffer);
                }
                Protocol::TCP => (),
                Protocol::UNKNOWN => (),
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
pub fn handle_icmp(buffer: &mut PacketBuffer) -> Option<PacketBuffer> {
    let icmp_packet = match buffer.parse::<IcmpPacket>() {
        Ok(x) => x,
        Err(_) => return None,
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
        Err(_) => return None,
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

/// Handle a UDP packet.
///
/// If this packet is destined for a socket and that socket has space in its
/// buffer, copy the packet data into the socket buffer.
pub fn handle_udp(buffer: &mut PacketBuffer) {
    let packet = match buffer.parse::<UdpPacket>() {
        Ok(x) => x,
        Err(_) => return,
    };

    // Is this packet destined for an active socket?
    let mut sockets = SOCKETS.lock();
    let socket_id = {
        let mut socket_id = None;
        for (k, v) in sockets.iter() {
            if Some(packet.dest_port()) == v.source_port {
                socket_id = Some(k);
                break;
            }
        }

        match socket_id {
            Some(id) => *id,
            None => return,
        }
    };

    let socket = match sockets.get_mut(&(socket_id as usize)) {
        Some(x) => x,
        None => panic!("socket not found\n\x00"),
    };

    // Do we have space in the socket buffer for the new data?
    if socket.buffer.len() + packet.data().len() >= BUFFER_SIZE {
        return;
    }
    socket.buffer.extend_from_slice(&packet.data());
}
