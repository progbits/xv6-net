use alloc::collections::BTreeMap;

use crate::ethernet::EthernetAddress;
use crate::ip::Ipv4Addr;

use crate::net::{FromBuffer, ToBuffer};
use crate::spinlock::Spinlock;

/// ARP Cache
static CACHE: Spinlock<Cache> = Spinlock::<Cache>::new(Cache(BTreeMap::new()));

/// An ARP cache entry.
struct CacheEntry {
    ethernet_addres: EthernetAddress,
    ip_address: Ipv4Addr,
}

struct Cache(BTreeMap<EthernetAddress, CacheEntry>);

impl Cache {
    pub fn address(ethernet_address: &EthernetAddress) -> Option<Ipv4Addr> {
        let cache = CACHE.lock();
        match cache.0.get(ethernet_address) {
            Some(x) => Some(x.ip_address.clone()),
            None => None,
        }
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
pub struct Packet {
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

impl Packet {
    pub fn from_slice(buf: &[u8]) -> Packet {
        Packet {
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
    pub fn from_request(request: &Packet, mac_address: EthernetAddress) -> Packet {
        Packet {
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

impl FromBuffer for Packet {
    fn from_buffer(buf: &[u8]) -> Packet {
        Packet::from_slice(&buf)
    }

    fn size(&self) -> usize {
        28
    }
}

impl ToBuffer for Packet {
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
