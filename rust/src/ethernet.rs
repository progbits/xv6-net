/// An ethernet (MAC) address.
#[derive(Debug)]
pub struct EthernetAddress([u8; 6]);

impl EthernetAddress {
    fn from_slice(buf: &[u8]) -> EthernetAddress {
        let mut addr: [u8; 6] = [0; 6];
        addr[..].copy_from_slice(buf);
        EthernetAddress(addr)
    }
}

/// The small subset of Ethertype values we care about.
#[derive(Debug)]
pub enum Ethertype {
    IPV4 = 0x0800,        // Internet Protocol version 4 (IPv4).
    ARP = 0x0806,         // Address Resolution Protocol (ARP).
    WAKE_ON_LAN = 0x0842, // Wake-on-LAN.
    RARP = 0x8035,        // Reverse Address Resolution Protocol (RARP).
    SLPP = 0x8103,        // Virtual Link Aggregation Control Protocol (VLACP).
    IPV6 = 0x86DD,        // Internet Protocol Version 6 (IPv6).
}

impl Ethertype {
    fn from_slice(buf: &[u8]) -> Option<Ethertype> {
        let mut raw: [u8; 2] = [0; 2];
        raw.clone_from_slice(&buf);
        match u16::from_be_bytes(raw) {
            0x0800 => Some(Ethertype::IPV4),
            0x0806 => Some(Ethertype::ARP),
            0x0842 => Some(Ethertype::WAKE_ON_LAN),
            0x8035 => Some(Ethertype::RARP),
            0x8103 => Some(Ethertype::SLPP),
            0x86DD => Some(Ethertype::IPV6),
            _ => None,
        }
    }
}

/// An ethernet frame header.
///
/// Represents an Ethernet frame header.
#[derive(Debug)]
#[repr(C)]
pub struct EthernetHeader {
    destination: EthernetAddress,
    source: EthernetAddress,
    ethertype: Ethertype,
}

impl EthernetHeader {
    pub fn new(
        source: EthernetAddress,
        destination: EthernetAddress,
        ethertype: Ethertype,
    ) -> EthernetHeader {
        EthernetHeader {
            source,
            destination,
            ethertype,
        }
    }

    /// Creates a new EthernetHeader from a slice of bytes.
    pub fn from_slice(buf: &[u8]) -> EthernetHeader {
        EthernetHeader {
            destination: EthernetAddress::from_slice(&buf[0..6]),
            source: EthernetAddress::from_slice(&buf[6..12]),
            ethertype: Ethertype::from_slice(&buf[12..14]).unwrap(),
        }
    }
}
