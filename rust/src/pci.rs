use crate::asm::{in_dw, out_dw};

/// PCI I/O.
const PCI_CONFIG_ADDR: u16 = 0xCF8;
const PCI_CONFIG_DATA: u16 = 0xCFC;

/// Represents a PCI configuration space header.
pub struct PciConfig {
    base_addr: u32,
    vendor_id: u16,
    device_id: u16,
    command: u16,
    status: u16,
    revision_id: u8,
    class_code: [u8; 3],
    cache_line_size: u8,
    lat_timer: u8,
    header_type: u8,
    bist: u8,
    bar: [u32; 6],
}

impl PciConfig {
    /// Read a new PciConfig struct from a memory mapped I/O address.
    pub fn new(base_addr: u32) -> Result<PciConfig, ()> {
        unsafe {
            let vendor_id = Self::read_vendor_id(base_addr);
            let device_id = Self::read_device_id(base_addr);
            let bar_0 = Self::read_bar(base_addr, 0);

            Ok(PciConfig {
                base_addr: base_addr,
                vendor_id: vendor_id,
                device_id: device_id,
                command: 0,
                status: 0,
                revision_id: 0,
                class_code: [0u8; 3],
                cache_line_size: 0,
                lat_timer: 0,
                header_type: 0,
                bist: 0,
                bar: [bar_0, 0, 0, 0, 0, 0],
            })
        }
    }

    /// Return the vendor id associated with the device.
    pub fn vendor_id(&self) -> u16 {
        self.vendor_id
    }

    /// Return the device id associated with the device.
    pub fn device_id(&self) -> u16 {
        self.device_id
    }

    /// Return the value of the ith base address register.
    pub fn bar(&self, i: u8) -> u32 {
        self.bar[i as usize]
    }

    /// Set the device as a bus master.
    pub unsafe fn set_bus_master(&self) {
        let mut command: u32 = 0x0;
        let mut j = 1;
        for i in (4..=5).rev() {
            out_dw(PCI_CONFIG_ADDR, self.base_addr | i);
            let data = in_dw(PCI_CONFIG_DATA);
            command |= data << (j * 8);
            j -= 1;
        }

        // Set the bus master flag and write back the command register.
        command |= 1 << 2;
        out_dw(PCI_CONFIG_ADDR, self.base_addr | 4);
        out_dw(PCI_CONFIG_DATA, command);
    }

    /// Read a PCI vendor identifier.
    unsafe fn read_vendor_id(base_addr: u32) -> u16 {
        let mut result: u16 = 0x0;
        for i in (0..=1).rev() {
            out_dw(PCI_CONFIG_ADDR, base_addr | i);
            let data = in_dw(PCI_CONFIG_DATA);
            result |= (data as u16) << (i * 8);
        }
        result
    }

    /// Read a PCI device identifier.
    unsafe fn read_device_id(base_addr: u32) -> u16 {
        let mut result: u16 = 0x0;
        let mut j = 1;
        for i in (2..=3).rev() {
            out_dw(PCI_CONFIG_ADDR, base_addr | i);
            let data = in_dw(PCI_CONFIG_DATA);
            result |= (data as u16) << (j * 8);
            j -= 1;
        }
        result
    }

    /// Read the nth BAR register.
    unsafe fn read_bar(base_addr: u32, _n: u32) -> u32 {
        let mut result: u32 = 0x0;
        let mut j = 3;
        for i in (16..=19).rev() {
            out_dw(PCI_CONFIG_ADDR, base_addr | i);
            let data = in_dw(PCI_CONFIG_DATA);
            result |= data << (j * 8);
            j -= 1;
        }
        result
    }
}
