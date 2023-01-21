use alloc::format;
use alloc::vec;
use alloc::vec::Vec;

use crate::packet_buffer::{FromBuffer, ToBuffer};

/// Represents an ICMP echo packet.
#[derive(Debug, Clone)]
pub struct IcmpEchoMessage {
    pub r#type: Type,
    code: u8,
    checksum: u16,
    identifier: u16,
    sequence_number: u16,
    data: Vec<u8>,
}

impl IcmpEchoMessage {
    /// Build a new echo response from a request.
    pub fn from_request(req: IcmpEchoMessage) -> IcmpEchoMessage {
        IcmpEchoMessage {
            r#type: Type::EchoReply,
            code: 0,
            checksum: req.checksum,
            identifier: req.identifier,
            sequence_number: req.sequence_number,
            data: req.data,
        }
    }
}

/// Represents an ICMP packet.
#[derive(Debug, Clone)]
pub enum IcmpPacket {
    EchoMessage(IcmpEchoMessage),
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Type {
    EchoReply,
    Reserved,
    DestinationUnreachable,
    SourceQuench,
    RedirectMessage,
    EchoRequest,
    RouterAdvertisement,
    RouterSolicitation,
    TimeExceeded,
    ParameterProblem,
    Timestamp,
    TimestampReply,
    InformationRequest,
    InformationReply,
    AddressMaskRequest,
    AddressMaskReply,
    Traceroute,
    ExtendedEchoRequest,
    ExtendedEchoReply,
    Unknown,
}

impl Type {
    pub fn from_slice(buf: &[u8]) -> Type {
        match buf[0] {
            0x00u8 => Type::EchoReply,
            0x01u8 => Type::Reserved,
            0x02u8 => Type::Reserved,
            0x03u8 => Type::DestinationUnreachable,
            0x04u8 => Type::SourceQuench,
            0x08u8 => Type::EchoRequest,
            _ => Type::Unknown,
        }
    }

    pub fn as_bytes(&self) -> u8 {
        match self {
            Type::EchoReply => 0x00u8,
            Type::Reserved => 0x01u8,
            Type::Reserved => 0x02u8,
            Type::DestinationUnreachable => 0x03u8,
            Type::SourceQuench => 0x04u8,
            Type::EchoRequest => 0x08u8,
            Type::Unknown => panic!(),
            _ => panic!(),
        }
    }
}

impl IcmpPacket {
    pub fn from_slice(buf: &[u8]) -> IcmpPacket {
        let r#type = Type::from_slice(&buf[0..]);
        match r#type {
            Type::EchoReply | Type::EchoRequest => IcmpPacket::EchoMessage(IcmpEchoMessage {
                r#type: r#type,
                code: buf[1],
                checksum: u16::from_be_bytes([buf[2], buf[3]]),
                identifier: u16::from_be_bytes([buf[4], buf[5]]),
                sequence_number: u16::from_be_bytes([buf[6], buf[7]]),
                data: buf[8..].to_vec(),
            }),
            _ => panic!(),
        }
    }

    fn calculate_checksum(buf: &[u8]) -> u16 {
        let mut sum = 0u32;
        for i in (0..buf.len()).step_by(2) {
            let value = u16::from_be_bytes([buf[i], buf[i + 1]]);
            sum += value as u32;
        }

        let check = (sum >> 16) + (sum & 0xffff);
        let check = (check >> 16) + check;
        !(check as u16)
    }
}

impl FromBuffer for IcmpPacket {
    fn from_buffer(buf: &[u8]) -> Result<IcmpPacket, ()> {
        Ok(IcmpPacket::from_slice(&buf))
    }

    fn size(&self) -> usize {
        match self {
            IcmpPacket::EchoMessage(x) => 8 + x.data.len(),
            _ => panic!(),
        }
    }
}

impl ToBuffer for IcmpPacket {
    fn to_buffer(&self, buf: &mut [u8]) {
        match self {
            IcmpPacket::EchoMessage(x) => {
                buf[0..1].copy_from_slice(&[x.r#type.as_bytes()]);
                buf[1..2].copy_from_slice(&[x.code]);
                buf[2..4].copy_from_slice(&0u16.to_be_bytes());
                buf[4..6].copy_from_slice(&x.identifier.to_be_bytes());
                buf[6..8].copy_from_slice(&x.sequence_number.to_be_bytes());
                buf[8..8 + x.data.len()].copy_from_slice(&x.data[..]);

                let checksum = IcmpPacket::calculate_checksum(&buf[0..8 + x.data.len()]);
                buf[2..4].copy_from_slice(&(checksum.to_be_bytes()));
            }
            _ => panic!(),
        }
    }

    fn size(&self) -> usize {
        match self {
            IcmpPacket::EchoMessage(x) => 8 + x.data.len(),
            _ => panic!(),
        }
    }
}
