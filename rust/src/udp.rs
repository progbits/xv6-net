use alloc::vec::Vec;

use crate::packet_buffer::{FromBuffer, PacketBuffer, ToBuffer};

/// Represents a UDP packet header.
#[derive(Debug, Clone)]
pub struct UdpPacket {
    source_port: u16,
    dest_port: u16,
    len: u16,
    checksum: u16,
    data: Vec<u8>,
}

impl UdpPacket {
    pub fn new(source_port: u16, dest_port: u16, data: Vec<u8>) -> Self {
        UdpPacket {
            source_port: source_port,
            dest_port: dest_port,
            len: (data.len() + 8) as u16,
            checksum: 0,
            data: data,
        }
    }

    fn from_slice(buf: &[u8]) -> Result<UdpPacket, ()> {
        if buf.len() < 8 {
            // This can't be a valid UDP packet.
            return Err(());
        }

        let source_port = u16::from_be_bytes([buf[0], buf[1]]);
        let dest_port = u16::from_be_bytes([buf[2], buf[3]]);
        let len = u16::from_be_bytes([buf[4], buf[5]]);
        let checksum = u16::from_be_bytes([buf[6], buf[7]]);
        let data_len = (len - 8) as usize;
        let data = buf[8..(8 + data_len)].to_vec();

        Ok(UdpPacket {
            source_port,
            dest_port,
            len,
            checksum,
            data,
        })
    }

    pub fn dest_port(&self) -> u16 {
        return self.dest_port;
    }

    pub fn data(&self) -> &[u8] {
        return &self.data[..];
    }
}

impl FromBuffer for UdpPacket {
    fn from_buffer(buf: &[u8]) -> Result<UdpPacket, ()> {
        UdpPacket::from_slice(&buf)
    }

    fn size(&self) -> usize {
        8 + self.data.len()
    }
}

impl ToBuffer for UdpPacket {
    fn to_buffer(&self, buf: &mut [u8]) {
        buf[0..2].copy_from_slice(&self.source_port.to_be_bytes());
        buf[2..4].copy_from_slice(&self.dest_port.to_be_bytes());
        buf[4..6].copy_from_slice(&self.len.to_be_bytes());
        buf[6..8].copy_from_slice(&self.checksum.to_be_bytes());
        buf[8..8 + self.data.len()].copy_from_slice(&self.data[..]);
    }

    fn size(&self) -> usize {
        8 + self.data.len()
    }
}
