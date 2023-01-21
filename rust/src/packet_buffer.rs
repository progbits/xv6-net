use alloc::vec;
use alloc::vec::Vec;

pub static BUFFER_SIZE: usize = 2048;

/// Represents raw packet data.
///
/// TODO: Stack allocated buffer?
pub struct PacketBuffer {
    /// The raw packet data.
    buf: Vec<u8>,
    /// The size of the raw packet.
    size: usize,
    /// The number of bytes we have parsed so far into the buffer.
    offset: usize,
    /// Has the buffer been written to?
    written: bool,
}

impl PacketBuffer {
    /// Create a new buffer with the specified size.
    pub fn new(size: usize) -> PacketBuffer {
        PacketBuffer {
            buf: vec![0u8; size],
            size: size,
            offset: 0,
            written: false,
        }
    }

    /// Create a new buffer from the data provided.
    pub fn new_from_bytes(data: *const u8, size: usize) -> PacketBuffer {
        let mut packet_buffer = PacketBuffer {
            buf: vec![0u8; size],
            size: size,
            offset: 0,
            written: false,
        };
        unsafe {
            core::ptr::copy(data, packet_buffer.buf.as_mut_ptr(), size);
        }
        packet_buffer
    }

    /// Parse a new packet from the buffer.
    /// TODO: Zero-copy?
    pub fn parse<T: FromBuffer>(&mut self) -> Result<T, ()> {
        let value = match T::from_buffer(&self.buf[self.offset..]) {
            Ok(x) => x,
            Err(_) => return Err(()),
        };
        self.offset += value.size();
        Ok(value)
    }

    /// Serialize a new packet to the buffer.
    /// TODO: Zero-copy?
    pub fn serialize<T: ToBuffer>(&mut self, value: &T) {
        self.offset += value.size();
        self.written = true;
        let start = self.buf.len() - self.offset;
        let end = start + value.size();
        value.to_buffer(&mut self.buf[start..end]);
    }

    /// Return the size of the buffer.
    pub fn len(&self) -> usize {
        self.offset
    }

    /// Return a pointer to the underlying buffer.
    pub fn as_ptr(&self) -> *const u8 {
        if self.written {
            self.buf[self.buf.len() - self.offset..].as_ptr()
        } else {
            self.buf[..self.offset].as_ptr()
        }
    }
}

/// Represents a type that can be parsed from a PacketBuffer.
pub trait FromBuffer {
    /// Parse a new instance from a slice of bytes.
    fn from_buffer(buf: &[u8]) -> Result<Self, ()>
    where
        Self: Sized;

    /// The size of the parsed structure, not including any encapsulated data.
    fn size(&self) -> usize;
}

/// Represents a type that can be serialized to a PacketBuffer.
pub trait ToBuffer {
    /// Parse a new instance from a slice of bytes.
    fn to_buffer(&self, buf: &mut [u8]);

    /// The size of the serialized structure, including any encapsulated data.
    fn size(&self) -> usize;
}
