use crate::net::{FromBuffer, ToBuffer};

/// An ethernet (MAC) address.
#[derive(Debug, Copy, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct EthernetAddress([u8; 6]);

impl EthernetAddress {
    pub fn from_slice(buf: &[u8]) -> EthernetAddress {
        EthernetAddress([buf[0], buf[1], buf[2], buf[3], buf[4], buf[5]])
    }

    pub fn as_bytes(&self) -> [u8; 6] {
        self.0
    }
}

/// The small subset of Ethertype values we care about.
#[derive(Debug, Copy, Clone)]
pub enum Ethertype {
    IPV4 = 0x0800,        // Internet Protocol version 4 (IPv4).
    ARP = 0x0806,         // Address Resolution Protocol (ARP).
    WAKE_ON_LAN = 0x0842, // Wake-on-LAN.
    RARP = 0x8035,        // Reverse Address Resolution Protocol (RARP).
    SLPP = 0x8103,        // Virtual Link Aggregation Control Protocol (VLACP).
    IPV6 = 0x86DD,        // Internet Protocol Version 6 (IPv6).
    UNKNOWN = 0xFFFF,
}

impl Ethertype {
    pub fn from_slice(buf: &[u8]) -> Ethertype {
        let mut raw: [u8; 2] = [0; 2];
        raw.clone_from_slice(&buf);
        match u16::from_be_bytes(raw) {
            0x0800 => Ethertype::IPV4,
            0x0806 => Ethertype::ARP,
            0x0842 => Ethertype::WAKE_ON_LAN,
            0x8035 => Ethertype::RARP,
            0x8103 => Ethertype::SLPP,
            0x86DD => Ethertype::IPV6,
            _ => Ethertype::UNKNOWN,
        }
    }

    fn as_bytes(&self) -> [u8; 2] {
        match self {
            Ethertype::IPV4 => 0x0800u16.to_be_bytes(),
            Ethertype::ARP => 0x0806u16.to_be_bytes(),
            Ethertype::WAKE_ON_LAN => 0x0842u16.to_be_bytes(),
            Ethertype::RARP => 0x0842u16.to_be_bytes(),
            Ethertype::SLPP => 0x8035u16.to_be_bytes(),
            Ethertype::IPV6 => 0x8103u16.to_be_bytes(),
            Ethertype::UNKNOWN => 0xFFFFu16.to_be_bytes(),
        }
    }
}

/// An ethernet frame header.
///
/// Represents an Ethernet frame header.
#[derive(Debug)]
#[repr(C)]
pub struct EthernetFrame {
    pub destination: EthernetAddress,
    pub source: EthernetAddress,
    pub ethertype: Ethertype,
}

impl EthernetFrame {
    pub fn new(
        destination: EthernetAddress,
        source: EthernetAddress,
        ethertype: Ethertype,
    ) -> EthernetFrame {
        EthernetFrame {
            destination,
            source,
            ethertype,
        }
    }

    /// Creates a new EthernetHeader from a slice of bytes.
    pub fn from_slice(buf: &[u8]) -> EthernetFrame {
        EthernetFrame {
            destination: EthernetAddress::from_slice(&buf[0..6]),
            source: EthernetAddress::from_slice(&buf[6..12]),
            ethertype: Ethertype::from_slice(&buf[12..14]),
        }
    }
}

impl FromBuffer for EthernetFrame {
    fn from_buffer(buf: &[u8]) -> Result<EthernetFrame, ()> {
        Ok(EthernetFrame::from_slice(&buf))
    }

    fn size(&self) -> usize {
        14
    }
}

impl ToBuffer for EthernetFrame {
    fn to_buffer(&self, buf: &mut [u8]) {
        let mut tmp = [0u8; 14];
        tmp[0..6].copy_from_slice(&self.destination.as_bytes());
        tmp[6..12].copy_from_slice(&self.source.as_bytes());
        tmp[12..14].copy_from_slice(&self.ethertype.as_bytes());
        buf.copy_from_slice(&tmp);
    }

    fn size(&self) -> usize {
        14
    }
}
