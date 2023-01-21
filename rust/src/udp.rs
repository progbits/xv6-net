use alloc::vec::Vec;

use crate::packet_buffer::{FromBuffer, PacketBuffer, ToBuffer};

/// Represents a UDP packet header.
#[derive(Debug, Clone)]
pub struct UdpPacket {
    src_port: u16,
    dst_port: u16,
    len: u16,
    checksum: u16,
    data: Vec<u8>,
}

impl UdpPacket {
    fn from_slice(buf: &[u8]) -> Result<UdpPacket, ()> {
        if buf.len() < 8 {
            return Err(());
        }
        Ok(UdpPacket {
            src_port: u16::from_be_bytes([buf[0], buf[1]]),
            dst_port: u16::from_be_bytes([buf[2], buf[3]]),
            len: u16::from_be_bytes([buf[4], buf[5]]),
            checksum: u16::from_be_bytes([buf[6], buf[7]]),
            data: buf[8..].to_vec(),
        })
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
        buf[0..2].copy_from_slice(&self.src_port.to_be_bytes());
        buf[2..4].copy_from_slice(&self.dst_port.to_be_bytes());
        buf[4..6].copy_from_slice(&self.len.to_be_bytes());
        buf[6..8].copy_from_slice(&self.checksum.to_be_bytes());
        buf[8..8 + self.data.len()].copy_from_slice(&self.data[..]);
    }

    fn size(&self) -> usize {
        8 + self.data.len()
    }
}
