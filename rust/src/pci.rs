use crate::asm::{in_dw, out_dw};

/// PCI I/O.
const PCI_CONFIG_ADDR: u16 = 0xCF8;
const PCI_CONFIG_DATA: u16 = 0xCFC;

/// PCI configuration space header.
pub struct PciConfig {
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

/// Read a PCI vendor identifier.
pub unsafe fn read_vendor_id(base_addr: u32) -> u16 {
    let mut result: u16 = 0x0;
    for i in (0..=1).rev() {
        out_dw(PCI_CONFIG_ADDR, base_addr | i);
        let data = in_dw(PCI_CONFIG_DATA);
        result |= (data as u16) << (i * 8);
    }
    result
}

/// Read a PCI device identifier.
pub unsafe fn read_device_id(base_addr: u32) -> u16 {
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

/// Read the `n`th BAR register.
pub unsafe fn read_bar(device: u32, _n: u8) -> u32 {
    let mut result: u32 = 0x0;
    let mut j = 3;
    for i in (16..=19).rev() {
        out_dw(PCI_CONFIG_ADDR, (0x80000000 | device << 11) | i);
        let data = in_dw(PCI_CONFIG_DATA);
        result |= data << (j * 8);
        j -= 1;
    }
    result
}

/// Setup the device as a bus master.
pub unsafe fn set_bus_master(device: u32) {
    let mut command: u32 = 0x0;
    let mut j = 1;
    for i in (4..=5).rev() {
        out_dw(PCI_CONFIG_ADDR, (0x80000000 | device << 11) | i);
        let data = in_dw(PCI_CONFIG_DATA);
        command |= data << (j * 8);
        j -= 1;
    }

    // Set the bus master flag and write back the command register.
    command |= 1 << 2;
    out_dw(PCI_CONFIG_ADDR, (0x80000000 | device << 11) | 4);
    out_dw(PCI_CONFIG_DATA, command);
}
