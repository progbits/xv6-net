use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::format;

use crate::ethernet::{EthernetAddress, EthernetFrame, Ethertype};
use crate::ip::Ipv4Addr;

use crate::net::{FromBuffer, NetworkDevice, PacketBuffer, ToBuffer, PACKET_BUFFER_SIZE};
use crate::spinlock::Spinlock;

/// ARP Cache.
static ARP_CACHE: Spinlock<ArpCache> = Spinlock::<ArpCache>::new(ArpCache(BTreeMap::new()));

pub struct ArpCache(BTreeMap<Ipv4Addr, EthernetAddress>);

impl ArpCache {
    /// Return the hardware address, if it exists in the cache.
    pub fn hardware_address(protocol_address: &Ipv4Addr) -> Option<EthernetAddress> {
        let cache = ARP_CACHE.lock();
        let result = cache.0.get(protocol_address).copied();
        result
    }

    /// Add a new entry from an ARP reply.
    pub fn reply(arp_packet: ArpPacket) {
        match arp_packet.oper {
            Operation::Request | Operation::Unknown => return,
            Operation::Reply => (),
        }

        let mut cache = ARP_CACHE.lock();
        cache.0.insert(arp_packet.spa, arp_packet.sha);
    }

    /// Send a request to resolve a hardware address.
    pub fn resolve(protocol_address: &Ipv4Addr, device: &mut Box<dyn NetworkDevice>) {
        let mut packet_buffer = PacketBuffer::new(PACKET_BUFFER_SIZE);

        let broadcast_hardware_address =
            EthernetAddress::from_slice(&[0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF]);
        let arp_request = ArpPacket {
            htype: HardwareType::Ethernet,
            ptype: ProtocolType::Ipv4,
            hlen: 6,
            plen: 4,
            oper: Operation::Request,
            sha: device.hardware_address(),
            spa: device.protocol_address(),
            tha: broadcast_hardware_address,
            tpa: *protocol_address,
        };
        packet_buffer.serialize(&arp_request);

        let ethernet_frame = EthernetFrame::new(
            broadcast_hardware_address,
            device.hardware_address(),
            Ethertype::ARP,
        );
        packet_buffer.serialize(&ethernet_frame);

        device.send(packet_buffer);
    }
}

#[derive(Debug)]
pub enum HardwareType {
    Ethernet,
    Unknown,
}

impl HardwareType {
    fn from_slice(buf: &[u8]) -> HardwareType {
        match u16::from_be_bytes([buf[0], buf[1]]) {
            0x0001 => HardwareType::Ethernet,
            _ => HardwareType::Unknown,
        }
    }

    fn as_bytes(&self) -> [u8; 2] {
        match self {
            HardwareType::Ethernet => 0x0001u16.to_be_bytes(),
            HardwareType::Unknown => 0x0000u16.to_be_bytes(),
        }
    }
}

#[derive(Debug)]
pub enum ProtocolType {
    Ipv4,
    Unknown,
}

impl ProtocolType {
    fn from_slice(buf: &[u8]) -> ProtocolType {
        match u16::from_be_bytes([buf[0], buf[1]]) {
            0x0800 => ProtocolType::Ipv4,
            _ => ProtocolType::Unknown,
        }
    }

    fn as_bytes(&self) -> [u8; 2] {
        match self {
            ProtocolType::Ipv4 => 0x0800u16.to_be_bytes(),
            ProtocolType::Unknown => 0x0000u16.to_be_bytes(),
        }
    }
}

#[derive(Debug)]
pub enum Operation {
    Request,
    Reply,
    Unknown,
}

impl Operation {
    fn from_slice(buf: &[u8]) -> Operation {
        match u16::from_be_bytes([buf[0], buf[1]]) {
            0x0001 => Operation::Request,
            0x0002 => Operation::Reply,
            _ => Operation::Unknown,
        }
    }

    fn as_bytes(&self) -> [u8; 2] {
        match self {
            Operation::Request => 0x0001u16.to_be_bytes(),
            Operation::Reply => 0x0002u16.to_be_bytes(),
            Operation::Unknown => 0xFFFFu16.to_be_bytes(),
        }
    }
}

/// Represents an ARP packet.
#[derive(Debug)]
pub struct ArpPacket {
    pub htype: HardwareType,
    pub ptype: ProtocolType,
    pub hlen: u8,
    pub plen: u8,
    pub oper: Operation,
    pub sha: EthernetAddress,
    pub spa: Ipv4Addr,
    pub tha: EthernetAddress,
    pub tpa: Ipv4Addr,
}

impl ArpPacket {
    pub fn from_slice(buf: &[u8]) -> ArpPacket {
        ArpPacket {
            htype: HardwareType::from_slice(&buf),
            ptype: ProtocolType::from_slice(&buf[2..]),
            hlen: buf[4],
            plen: buf[5],
            oper: Operation::from_slice(&buf[6..]),
            sha: EthernetAddress::from_slice(&buf[8..]),
            spa: Ipv4Addr::from_slice(&buf[14..]),
            tha: EthernetAddress::from_slice(&buf[18..]),
            tpa: Ipv4Addr::from_slice(&buf[24..]),
        }
    }

    /// Create a new ARP response from a request.
    pub fn from_request(request: &ArpPacket, mac_address: EthernetAddress) -> ArpPacket {
        ArpPacket {
            htype: HardwareType::Ethernet,
            ptype: ProtocolType::Ipv4,
            hlen: 0x06,
            plen: 0x04,
            oper: Operation::Reply,
            sha: mac_address,
            spa: request.tpa,
            tha: request.sha,
            tpa: request.spa,
        }
    }
}

impl FromBuffer for ArpPacket {
    fn from_buffer(buf: &[u8]) -> Result<ArpPacket, ()> {
        Ok(ArpPacket::from_slice(&buf))
    }

    fn size(&self) -> usize {
        28
    }
}

impl ToBuffer for ArpPacket {
    fn to_buffer(&self, buf: &mut [u8]) {
        let mut tmp = [0u8; 28];
        tmp[0..2].copy_from_slice(&self.htype.as_bytes());
        tmp[2..4].copy_from_slice(&self.ptype.as_bytes());
        tmp[4] = self.hlen;
        tmp[5] = self.plen;
        tmp[6..8].copy_from_slice(&self.oper.as_bytes());
        tmp[8..14].copy_from_slice(&self.sha.as_bytes());
        tmp[14..18].copy_from_slice(&self.spa.as_bytes());
        tmp[18..24].copy_from_slice(&self.tha.as_bytes());
        tmp[24..28].copy_from_slice(&self.tpa.as_bytes());
        buf.copy_from_slice(&tmp);
    }

    fn size(&self) -> usize {
        28
    }
}
