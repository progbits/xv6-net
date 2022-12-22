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
