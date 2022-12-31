use crate::net::FromBuffer;

/// An IPv4 address.
#[derive(Debug, Copy, Clone, Eq, Ord, PartialEq, PartialOrd)]
pub struct Ipv4Addr([u8; 4]);

impl Ipv4Addr {
    /// Construct a new Ipv4Addr from its components.
    pub fn new(a: u8, b: u8, c: u8, d: u8) -> Ipv4Addr {
        Ipv4Addr([a, b, c, d])
    }

    pub fn from_slice(buf: &[u8]) -> Ipv4Addr {
        Ipv4Addr([buf[0], buf[1], buf[2], buf[3]])
    }

    /// Return the byte representation of an Ipv4Addr.
    pub fn as_bytes(&self) -> [u8; 4] {
        self.0
    }
}

#[derive(Debug, Copy, Clone)]
pub enum Protocol {
    ICMP = 0x01,
    TCP = 0x06,
    UDP = 0x11,
    UNKNOWN = 0xFF,
}

impl Protocol {
    pub fn from_slice(buf: &[u8]) -> Protocol {
        match buf[0] {
            0x01u8 => Protocol::ICMP,
            0x06u8 => Protocol::TCP,
            0x11u8 => Protocol::UDP,
            _ => Protocol::UNKNOWN,
        }
    }

    fn as_bytes(&self) -> u8 {
        match self {
            Protocol::ICMP => 0x01u8,
            Protocol::TCP => 0x06u8,
            Protocol::UDP => 0x11u8,
            Protocol::UNKNOWN => 0xFFu8,
        }
    }
}

/// An IPv4 packet.
///
/// Represents an IPV4 packet header without options.
///
/// RFC791 Section 3.1
/// https://tools.ietf.org/html/rfc791
#[derive(Debug)]
pub struct Ipv4Packet {
    version: u8,
    header_length: u8,
    dscp: u8,
    ecn: u8,
    total_length: u16,
    identification: u16,
    df: bool,
    mf: bool,
    fragment_offset: u16,
    time_to_live: u8,
    protocol: Protocol,
    header_checksum: u16,
    source_address: Ipv4Addr,
    destination_address: Ipv4Addr,
}

impl Ipv4Packet {
    /// Creates a new Ipv4Header with the specified values.
    ///
    /// Headers are created with their checksum set to 0. Checksums are calculated on write.
    pub fn new(
        dscp: u8,
        ecn: u8,
        total_length: u16,
        identification: u16,
        df: bool,
        mf: bool,
        fragment_offset: u16,
        time_to_live: u8,
        protocol: Protocol,
        source_address: Ipv4Addr,
        destination_address: Ipv4Addr,
    ) -> Ipv4Packet {
        Ipv4Packet {
            version: 4,
            header_length: 5,
            dscp: dscp,
            ecn: ecn,
            total_length: total_length,
            identification: identification,
            df: df,
            mf: mf,
            fragment_offset: fragment_offset,
            time_to_live: time_to_live,
            protocol: protocol,
            header_checksum: 0,
            source_address: source_address,
            destination_address: destination_address,
        }
    }

    /// Creates a new Ipv4Header from a slice of bytes.
    pub fn from_slice(buf: &[u8]) -> Ipv4Packet {
        Ipv4Packet {
            version: buf[0] >> 4,
            header_length: buf[0] & 0xf,
            dscp: buf[1] & 0xfc,
            ecn: buf[1] & 0x3,
            total_length: u16::from_be_bytes([buf[2], buf[3]]),
            identification: u16::from_be_bytes([buf[4], buf[5]]),
            df: buf[6] & 0x40 > 0,
            mf: buf[6] & 0x20 > 0,
            fragment_offset: u16::from_be_bytes([buf[6] & 0x1f, buf[7]]),
            time_to_live: buf[8],
            protocol: Protocol::from_slice(&buf[9..]),
            header_checksum: u16::from_be_bytes([buf[10], buf[11]]),
            source_address: Ipv4Addr::new(buf[12], buf[13], buf[14], buf[15]),
            destination_address: Ipv4Addr::new(buf[16], buf[17], buf[18], buf[19]),
        }
    }

    pub fn protocol(&self) -> Protocol {
        self.protocol
    }

    /// Write the header to `buf` with the appropriate checksum.
    ///
    /// The header is written to a stack allocated buffer, the checksum calculated, then the header is written to `buf`.
    pub fn write(&self, buf: &mut [u8]) {
        // Temporary write buffer to make calculating the checksum easier.
        let mut bytes = [0u8; 20];

        // Version and length.
        bytes[0] = {
            let mut byte = 0u8;
            byte |= self.version & 0xf;
            byte <<= 4;
            byte |= self.header_length & 0xf;
            byte
        };

        // DSCP and ECN.
        bytes[1] = {
            let mut byte = 0u8;
            byte |= self.dscp & 0b11111100;
            byte |= self.ecn & 0b00000011;
            byte
        };

        // Total length and identification.
        bytes[2..4].copy_from_slice(&self.total_length.to_be_bytes());
        bytes[4..6].copy_from_slice(&self.identification.to_be_bytes());

        // Flags and fragment offset.
        bytes[6..8].copy_from_slice(
            &{
                let mut half = 0u16;
                half |= (self.df as u16) << 14;
                half |= (self.mf as u16) << 14;
                half |= self.fragment_offset & 0x0FFF;
                half
            }
            .to_be_bytes(),
        );

        // Time to live and protocol.
        bytes[8] = self.time_to_live;
        bytes[9] = self.protocol.as_bytes();

        // Skip the checksum until we have written the rest of the header.
        bytes[10..12].copy_from_slice(&0u16.to_be_bytes());

        // Source and destination address.
        bytes[12..16].copy_from_slice(&self.source_address.as_bytes());
        bytes[16..20].copy_from_slice(&self.destination_address.as_bytes());

        // Now `bytes` contains the complete header, calculate the checksum.
        let checksum = &Ipv4Packet::calculate_checksum(&bytes);
        bytes[10..12].copy_from_slice(&checksum.to_be_bytes());

        // Write the temporary buffer.
        buf.copy_from_slice(&bytes);
    }

    /// Calculates the checksum for a slice of bytes.
    ///
    /// RFC791 Section 3.1
    /// https://tools.ietf.org/html/rfc791
    fn calculate_checksum(buf: &[u8]) -> u16 {
        let mut sum = 0u32;
        for i in (0..20).step_by(2) {
            let value = u16::from_be_bytes([buf[i], buf[i + 1]]);
            sum += value as u32;
        }

        let check = (sum >> 16) + (sum & 0xffff);
        let check = (check >> 16) + check;
        !(check as u16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write() {
        let header = Ipv4Header::new(
            0,
            0,
            60,
            4889,
            true,
            false,
            0,
            64,
            6,
            Ipv4Addr::new(10, 0, 0, 1),
            Ipv4Addr::new(10, 0, 0, 2),
        );

        let mut buf = [0u8; 20];
        let mut slice = &mut buf[..];
        header.write(&mut slice);
        assert_eq!(
            buf,
            [
                0x45, 0x0, 0x0, 0x3c, 0x13, 0x19, 0x40, 0x0, 0x40, 0x6, 0x13, 0xa1, 0xa, 0x0, 0x0,
                0x1, 0xa, 0x0, 0x0, 0x2,
            ]
        );
    }

    #[test]
    fn test_from_slice() {
        let header = Ipv4Header::from_slice(&[
            0x45, 0x0, 0x0, 0x3c, 0x13, 0x19, 0x40, 0x0, 0x40, 0x6, 0x13, 0xa1, 0xa, 0x0, 0x0, 0x1,
            0xa, 0x0, 0x0, 0x2,
        ]);

        assert_eq!(header.version, 4);
        assert_eq!(header.header_length, 5);
        assert_eq!(header.dscp, 0);
        assert_eq!(header.ecn, 0);
        assert_eq!(header.total_length, 60);
        assert_eq!(header.identification, 4889);
        assert_eq!(header.df, true);
        assert_eq!(header.mf, false);
        assert_eq!(header.fragment_offset, 0);
        assert_eq!(header.time_to_live, 64);
        assert_eq!(header.protocol, 6);
        assert_eq!(header.header_checksum, 5025);
        assert_eq!(header.source_address, Ipv4Addr::new(10, 0, 0, 1));
        assert_eq!(header.destination_address, Ipv4Addr::new(10, 0, 0, 2));
    }
}

impl FromBuffer for Ipv4Packet {
    fn from_buffer(buf: &[u8]) -> Ipv4Packet {
        Ipv4Packet::from_slice(&buf)
    }

    fn size(&self) -> usize {
        40
    }
}
