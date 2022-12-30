///
/// An interrupt driven E1000 network card driver.
///
use alloc::format;
use alloc::vec;
use alloc::vec::Vec;
use core::ptr;
use core::slice;

use crate::ethernet::EthernetAddress;
use crate::kernel::{cprint, ioapicenable, kalloc};
use crate::mm::{PhysicalAddress, VirtualAddress, PAGE_SIZE};
use crate::net::{handle_packet, NetworkDevice, PacketBuffer};
use crate::pci;
use crate::spinlock::Spinlock;

const IRQ_PIC0: u32 = 0xB;

const EEPROM_DONE: u32 = 0x00000010;

// Device identifiers.
const VENDOR_ID: u16 = 0x8086; // Intel.
const DEVICE_ID: u16 = 0x100E; // 82540EM Gigabit Ethernet Controller.

// E1000 device registers.
enum DeviceRegister {
    CTRL = 0x00000,
    STATUS = 0x00008,
    EERD = 0x0014,
    ICR = 0x000C0,
    IMS = 0x000D0,
    RCTL = 0x00100,
    TIPG = 0x00410,
    RDBAL = 0x02800,
    RDBAH = 0x02804,
    RDLEN = 0x02808,
    RDH = 0x02810,
    RDT = 0x02818,
    TDFPC = 0x03430,
    TDBAL = 0x03800,
    TDBAH = 0x03804,
    TDLEN = 0x03808,
    TDH = 0x03810,
    TDT = 0x03818,
    TCTL = 0x00400,
    GPTC = 0x04080,
    TPT = 0x040D4,
    RAL = 0x05400,
    RAH = 0x05404,
    MTA_LOW = 0x05200,
    MTA_HIGH = 0x053FC,
    PBM_START = 0x10000,
}

enum InterruptMask {
    /// Transmit Descriptor Written Back
    TXDW = 1 << 0,
    /// Transmit Queue Empty
    TXQE = 1 << 1,
    /// Link Status Change
    LSC = 1 << 2,
    /// Receive Sequence Error
    RXSEQ = 1 << 3,
    /// Receive Descriptor Minimum Threshold Reached
    RXDMTO = 1 << 4,
    /// Receiver Overrun
    RXO = 1 << 6,
    /// Receiver Timer Interrupt
    RXT0 = 1 << 7,
}

/// The receive descriptor.
#[repr(C)]
#[derive(Debug, Default)]
struct RxDesc {
    /// The address of the buffer backing this receive descriptor.
    addr: PhysicalAddress,
    /// Padding, as our addresses are only 32b.
    pad: [u8; 4],
    length: [u8; 2],
    checksum: [u8; 2],
    status: u8,
    errors: u8,
    special: [u8; 2],
}

impl RxDesc {
    fn packet_size(&self) -> usize {
        u16::from_le_bytes(self.length) as usize
    }

    /// Is the ed of packet (EOP) flag set?
    fn end_of_packet(&self) -> bool {
        self.status & (1 << 1) > 0
    }
}

/// The transmit descriptor.
#[repr(C)]
#[derive(Debug, Default)]
struct TxDesc {
    /// The address of the buffer backing this receive descriptor.
    addr: PhysicalAddress,
    /// Padding, as our addresses are only 32b.
    pad: [u8; 4],
    options: [u32; 2],
}

/// A representation of the e1000 family device state.
pub struct E1000 {
    mmio_base: u32,
    mac_addr: Option<EthernetAddress>,
    rx: Vec<RxDesc>,
    rx_idx: u32,
    tx: Vec<TxDesc>,
    tx_idx: u32,
}

impl E1000 {
    /// Initialize a new driver instance for an E1000 family ethernet card.
    ///
    /// By the end of this method, if successful, we will have:
    ///
    ///  - Numberated the PCI bus and located an attached Intel 8254x family ethernet card
    ///  - Stored the MMIO base address
    ///  - Stored the EEPROM based MAC address
    ///  - Configured the card as a bus master
    ///  - Setup receive functions
    ///  - Setup transmit functions
    ///  - Setup interrupts
    ///
    /// When reading the PCI configuration space, It is assumed that the memory
    /// mapped address is held in the first BAR register.
    pub unsafe fn new() -> Option<E1000> {
        let mut e1000 = E1000 {
            mmio_base: 0x0,
            mac_addr: None,
            rx: vec![],
            rx_idx: 0,
            tx: vec![],
            tx_idx: 0,
        };

        // Enumerate the first four devices on the first PCI bus.
        // TODO: Move this out to a more generic PCI `probe` routine.
        let mut target_device: Option<u32> = None;
        for device in 0..4 {
            let device_addr: u32 = 0x80000000 | (device << 11);

            // Read the vendor and device id of the current device.
            let vendor_id = pci::read_vendor_id(device_addr);
            let device_id = pci::read_device_id(device_addr);
            if vendor_id == VENDOR_ID && device_id == DEVICE_ID {
                target_device = Some(device);
                break;
            }
        }

        if target_device.is_none() {
            cprint(b"failed to locate network device\n\x00".as_ptr());
            panic!();
        }

        // Configure the device command register and read the MMIO base register.
        pci::set_bus_master(target_device.unwrap());
        e1000.mmio_base = pci::read_bar(target_device.unwrap(), 0);

        // Read the MAC address.
        // TODO: Lock EEPROM.
        let eerd_ptr = e1000.mmio_base + DeviceRegister::EERD as u32;
        let mut mac_addr = [0u8; 6];
        for i in 0..3 {
            core::ptr::write_volatile(eerd_ptr as *mut u32, 0x00000001 | i << 8);
            let mut data = core::ptr::read_volatile(eerd_ptr as *const u32);
            while (data & EEPROM_DONE) == 0 {
                data = core::ptr::read_volatile(eerd_ptr as *const u32);
            }
            data >>= 16;

            mac_addr[(i * 2) as usize] = (data & 0xFF as u32) as u8;
            mac_addr[(i * 2 + 1) as usize] = (data >> 8 & 0xFF as u32) as u8;
        }
        e1000.mac_addr = Some(EthernetAddress::from_slice(&mac_addr));

        // Setup receive functionality.
        e1000.init_rx();

        // Setup trasmit functionality.
        e1000.init_tx();

        // Setup interrupts.
        e1000.init_interrupts();

        // Register the network device with the network stack.

        // Enable interrupts.
        // TODO: Parse APICs tables to determine interrupts.
        ioapicenable(IRQ_PIC0, 0);

        Some(e1000)
    }

    /// Receive initialization.
    ///
    /// Reference: Manual - Section 14.4
    ///
    /// - Program receive address registers with MAC address.
    /// - Zero out the multicast table array.
    /// - Allocate a buffer to hold receive descriptors.
    /// - Setup the receive controller register.
    unsafe fn init_rx(&mut self) {
        // Write the MAC addres into the RAL and RAH registers.
        // Pad the MAC address to 8 bytes.
        match &self.mac_addr {
            Some(x) => {
                let mut mac_padded: [u8; 8] = [0; 8];
                mac_padded[..6].clone_from_slice(&x.as_bytes());

                // Copy out the low...
                let mac_low: u32 = u32::from_le_bytes(mac_padded[..4].try_into().unwrap());
                self.write_register(DeviceRegister::RAL, mac_low);

                // ...and high bytes of the MAC address.
                let mac_high: u32 = u32::from_le_bytes(mac_padded[4..].try_into().unwrap());
                self.write_register(DeviceRegister::RAH, mac_high);
            }
            None => panic!(),
        }
        // Allocate a recieve buffer for each of the descriptors.
        self.rx.resize_with(256, Default::default);
        for desc in self.rx.iter_mut() {
            let buf = VirtualAddress::new(kalloc() as *mut u8 as u32);
            desc.addr = buf.to_physical_address();
        }

        // Setup the receive descriptor buffer registers.
        let rx_buf = VirtualAddress::new(self.rx.as_ptr() as u32);
        self.write_register(DeviceRegister::RDBAL, rx_buf.to_physical_address().value());
        self.write_register(DeviceRegister::RDBAH, 0x0);
        self.write_register(DeviceRegister::RDLEN, PAGE_SIZE as u32);
        self.write_register(DeviceRegister::RDH, 0);
        // Point the receive descriptor tail one past the last valid descriptor.
        self.write_register(DeviceRegister::RDT, (self.rx.len() - 1) as u32);

        // Set up the receive control register.
        let mut rctl: u32 = 0x0;
        rctl |= 1 << 1; // Receiver enable.
        rctl |= 1 << 2; // Store bad packets.
        rctl |= 1 << 3; // Receive all unicast packets.
        rctl |= 1 << 4; // Receive all multicast packets.
        rctl |= 1 << 5; // Receivce long packets.
        rctl |= 1 << 15; // Accept broadcast packets.
        rctl |= 3 << 16; // Buffer size (4069 bytes).
        rctl |= 1 << 25; // Buffer size extension.
        self.write_register(DeviceRegister::RCTL, rctl);
    }

    /// Transmission initialization.
    ///
    /// Reference: Manual - Section 14.5
    ///
    /// - Allocate a buffer to hold transmission descriptors.
    /// - Initialize the transmit descriptor buffer registers.
    /// - Setup the transmission control register.
    /// - Setup the transmission inter-packet gap register.

    unsafe fn init_tx(&mut self) {
        // Allocate the transmission data buffer list and then for each transmission
        // descriptor, allocate a data buffer and write the descriptor.
        self.tx.resize_with(256, Default::default);
        for desc in self.tx.iter_mut() {
            let buf = VirtualAddress::new(kalloc() as *mut u8 as u32);
            desc.addr = buf.to_physical_address();
        }
        self.tx_idx = 1;

        // Setup the transmit descriptor buffer registers.
        let tx_buf = VirtualAddress::new(self.tx.as_ptr() as u32);
        self.write_register(DeviceRegister::TDBAL, tx_buf.to_physical_address().value());
        self.write_register(DeviceRegister::TDBAH, 0x0);
        self.write_register(DeviceRegister::TDLEN, PAGE_SIZE as u32);
        self.write_register(DeviceRegister::TDH, 0);
        self.write_register(DeviceRegister::TDT, 0);

        // Setup the transmission control TCTL register.
        let mut tctl: u32 = 0x0;
        tctl |= 1 << 1; // Transmit enable.
        tctl |= 1 << 3; // Pad short packets
        tctl |= 0x0F << 4; // Collision threshold.
        tctl |= 0x200 << 12; // Collision distance.
        self.write_register(DeviceRegister::TCTL, tctl);

        // Setup the transmission inter-packet gap (TIPG) register.
        self.write_register(DeviceRegister::TIPG, 0xA);
    }

    /// Configure interrupts.
    unsafe fn init_interrupts(&mut self) {
        let mut ims: u32 = 0x0;
        ims |= 1 << 0;
        ims |= 1 << 2;
        ims |= 1 << 3;
        ims |= 1 << 4;
        ims |= 1 << 6;
        ims |= 1 << 7;
        self.write_register(DeviceRegister::IMS, ims);
    }

    /// Read a device register.
    unsafe fn read_register(&self, r: DeviceRegister) -> u32 {
        return core::ptr::read_volatile((self.mmio_base + r as u32) as *const u32);
    }

    /// Write a device register.
    unsafe fn write_register(&self, r: DeviceRegister, data: u32) {
        core::ptr::write_volatile((self.mmio_base + r as u32) as *mut u32, data);
    }

    /// Return the hardware adddress of the network device.
    fn mac_address(&self) -> Option<EthernetAddress> {
        self.mac_addr.clone()
    }
}

/// Implement the common network interface.
impl NetworkDevice for E1000 {
    fn mac_address(&self) -> EthernetAddress {
        self.mac_addr.unwrap()
    }

    /// Clear interrupt register.
    fn clear_interrupts(&mut self) {
        // Read the interrupt register and dispatch to the correct handler.
        unsafe {
            let mask = self.read_register(DeviceRegister::ICR);
            if mask & InterruptMask::TXDW as u32 != 0 {
                // cprint(b"e1000: tx descriptor write-back\n\x00".as_ptr());
            } else if mask & InterruptMask::TXQE as u32 != 0 {
                // cprint(b"e1000: tx queue empty\n\x00".as_ptr());
            } else if mask & InterruptMask::LSC as u32 != 0 {
                // cprint(b"e1000: link status change seq error\n\x00".as_ptr());
            } else if mask & InterruptMask::RXSEQ as u32 != 0 {
                // cprint(b"e1000: rx seq error\n\x00".as_ptr());
            } else if mask & InterruptMask::RXDMTO as u32 != 0 {
                // cprint(b"e1000: rx min threshold\n\x00".as_ptr());
            } else if mask & InterruptMask::RXO as u32 != 0 {
                panic!();
            } else if mask & InterruptMask::RXT0 as u32 != 0 {
                // cprint(b"e1000: rx min threshold\n\x00".as_ptr());
            }
        }
    }

    fn send(&mut self, buf: PacketBuffer) {
        let mut tx_desc = &mut self.tx[self.tx_idx as usize];

        // Write the payload into the transmit buffer.
        let mut tx_buf = tx_desc.addr.to_virtual_address().value() as *mut u8;
        unsafe {
            core::ptr::copy(buf.as_bytes().as_ptr(), tx_buf, buf.len());
        }

        // Setup the transmit descriptor.
        let size = buf.len() as u32;
        let dtyp = 1u32 << 0;
        let dcmd = (1u32 << 0) | (1u32 << 3) | (1u32 << 5);
        tx_desc.options[0] = size | (dtyp << 20) | (dcmd << 24);

        self.tx_idx += 1;
        if (self.tx_idx as usize == self.tx.len()) {
            self.tx_idx = 0;
        }
        unsafe {
            self.write_register(DeviceRegister::TDT, self.tx_idx);
        }
    }

    /// Read avaliable packets from the device.
    ///
    /// TODO:
    /// 	- Handle fragmented packets.
    /// 	- Loan out PacketBuffers
    fn recv(&mut self) -> Option<PacketBuffer> {
        unsafe {
            let head = self.read_register(DeviceRegister::RDH);
            if self.rx_idx == head {
                // Ring buffer is empty.
                return None;
            }
        }

        let desc = &self.rx[self.rx_idx as usize];
        if !desc.end_of_packet() {
            panic!();
        }

        self.rx_idx += 1;
        if self.rx_idx == self.rx.len() as u32 {
            self.rx_idx = 0;
        }

        unsafe {
            self.write_register(DeviceRegister::RDT, self.rx_idx - 1);
        }

        Some(PacketBuffer::new_from_bytes(
            desc.addr.to_virtual_address().value() as *const u8,
            desc.packet_size(),
        ))
    }
}
