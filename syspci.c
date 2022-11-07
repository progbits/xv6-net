//
// PCI system calls
//

#include "types.h"
#include "x86.h"
#include "defs.h"
#include "memlayout.h"
#include "traps.h"

// PCI Constants.
const short PCI_CONFIG_ADDR = 0xCF8;
const short PCI_CONFIG_DATA = 0xCFC;

// Device constants.
const ushort VENDOR_ID = 0x8086; // Intel
const ushort DEVICE_ID = 0x100E; // 82540EM Gigabit Ethernet Controller

// E1000 registers.
const uint CTRL = 0x00000;
const uint STATUS = 0x00008;
const ushort EERD = 0x0014;
const uint ICR = 0x000C0;
const uint IMS = 0x000D0;
const uint RCTL = 0x00100;
const uint TIPG = 0x00410;
const uint RDBAL = 0x02800;
const uint RDBAH = 0x02804;
const uint RDLEN = 0x02808;
const uint RDH = 0x02810;
const uint RDT = 0x02818;
const uint TDFPC = 0x03430;
const uint TDBAL = 0x03800;
const uint TDBAH = 0x03804;
const uint TDLEN = 0x03808;
const uint TDH = 0x03810;
const uint TDT = 0x03818;
const uint TCTL = 0x00400;
const uint GPTC = 0x04080;
const uint TPT = 0x040D4;
const uint RAL = 0x05400;
const uint RAH = 0x05404;
const uint MTA_LOW = 0x05200;
const uint MTA_HIGH = 0x053FC;
const uint PBM_START = 0x10000;

const uint N_RX_DESC = (1 << 12) / 16;

static struct e1000 {
  uint mmio_base; // The base address of the cards MMIO region.
  char mac[6];    // The cards EEPROM configured MAC address.
  char *rx_buf;   // Page sized buffer holding recieve descriptors.
  char **rx_data; // List of page sized receive data buffers.
  char *tx_buf;   // Page sized buffer holding transmit descriptors.
} e1000;

// Represents the receive descriptor.
//
// Section 3.2.3 Receive Descriptor Format.
struct rx_desc {
  ulong addr;
  ulong fields;
} rx_desc;

// Read a main function register.
uint read_reg(uint reg) { return *(uint *)(e1000.mmio_base + reg); }

// Write a main function register.
void write_reg(uint reg, uint value) {
  *(uint *)(e1000.mmio_base + reg) = value;
}

// Initialize an E1000 family ethernet card.
//
// By the end of this method, if successful, we will have located an attached
// Intel 8254x family ethernet card, recorded its MMIO base address and EEPROM
// based MAC address.
//
// When reading the PCI configuration space, It is assumed that the memory
// mapped address is held in the first BAR register.
//
// As we are reading the PCI configuration space, we also ensure the
// card is configured to act as a bus master for DMA.
void init() {
  const uint EEPROM_DONE = 0x00000010;

  // Because we tightly control the environment, assume that the ethernet
  // controller is on one of the first 4 pci devices on the first bus.
  int target_dev = -1; // Set when the target device is found.
  for (int dev = 0; dev < 4; dev++) {
    const uint addr = 0x80000000 | dev << 11;

    ushort vendor_id = 0x0;
    for (int i = 1; i >= 0; i--) {
      outdw(PCI_CONFIG_ADDR, addr | i);
      uchar data = inb(PCI_CONFIG_DATA);
      vendor_id = vendor_id | (data << (i * 8));
    }

    ushort device_id = 0x0;
    for (int i = 3, j = 1; i >= 2; i--, j--) {
      outdw(PCI_CONFIG_ADDR, addr | i);
      uchar data = inb(PCI_CONFIG_DATA);
      device_id = device_id | (data << (j * 8));
    }

    // Check vendor and device identifiers.
    if (vendor_id == VENDOR_ID && device_id == DEVICE_ID) {
      target_dev = dev;
      break;
    }
  }

  if (target_dev == -1) {
    // Failed to find an 8254x family card.
    return 0;
  }

  // Read the current command register, set the bus master bit and write back
  // the command register.
  uint command = 0x0;
  for (int i = 5, j = 1; i >= 4; i--, j--) {
    outdw(PCI_CONFIG_ADDR, (0x80000000 | target_dev << 11) | i);
    uchar data = inb(PCI_CONFIG_DATA);
    command = command | (data << (j * 8));
  }
  command |= 1 << 2;
  outdw(PCI_CONFIG_ADDR, (0x80000000 | target_dev << 11) | 4);
  outdw(PCI_CONFIG_DATA, command);

  // Assume the address we want is in the first BAR register.
  uint mmio_addr = 0x0;
  for (int i = 19, j = 3; i >= 16; i--, j--) {
    outdw(PCI_CONFIG_ADDR, (0x80000000 | target_dev << 11) | i);
    uchar data = inb(PCI_CONFIG_DATA);
    mmio_addr = mmio_addr | (data << (j * 8));
  }

  if (mmio_addr == 0) {
    panic("failed to determine base address");
  }
  e1000.mmio_base = mmio_addr;

  // MAC address is stored in first 6 bytes of EEPROM.
  for (int i = 0; i < 3; i++) {
    volatile uint addr = e1000.mmio_base + EERD;
    *(uint *)(addr) = 0x00000001 | i << 8;
    uint result = 0x0;
    while (!(result & EEPROM_DONE)) {
      result = *(uint *)(addr);
    };
    ushort part = (*(uint *)(addr)) >> 16;
    memcpy(e1000.mac + i * sizeof(ushort), &part, sizeof(ushort));
  }
}

// Recieve initialization.
//
// Reference: Manual - Section 14.4
//
// - Program recieve address registers with MAC address.
// - Zero out the multicast table array.
// - Allocate a buffer to hold recieve descriptors.
// - Setup the recieve controler register.
void init_rx() {
  // Write MAC address.
  uint mac_low = 0x0;
  uint mac_high = 0x0;
  memcpy(&mac_low, e1000.mac, 4);
  memcpy(&mac_high, e1000.mac + 4, 2);
  write_reg(RAL, mac_low);
  write_reg(RAH, mac_high);

  // Recieve descriptor buffer should be 16B aligned. Its page aligned,
  // so this is fine
  e1000.rx_buf = kalloc();
  if (e1000.rx_buf == 0) {
    panic("failed to allocate recieve descriptor buffer\n");
  }
  memset(e1000.rx_buf, 0, 1 << 12);

  // Setup the recieve descriptor buffer registers.
  write_reg(RDBAL, V2P(e1000.rx_buf));
  write_reg(RDBAH, 0x0);
  write_reg(RDLEN, 1 << 12);
  write_reg(RDH, 0);
  write_reg(RDT, 0);

  // Allocate the receive data buffer list and then for each receive descriptor,
  // allocate a data buffer and write the descriptor.
  e1000.rx_data = kalloc();
  for (uint i = 0; i < N_RX_DESC; i++) {
    e1000.rx_data[i] = kalloc();
    if (e1000.rx_data[i] == 0) {
      panic("failed to allocate buffer\n");
    }
    memset(e1000.rx_data[i], 0, 1 << 12);

    // Write the descriptor for the buffer.
    struct rx_desc desc = {
        .addr = V2P(e1000.rx_data[i]),
    };
    memcpy(e1000.rx_buf + (i * sizeof(struct rx_desc)), &desc,
           sizeof(struct rx_desc));
  }
  write_reg(RDT, N_RX_DESC);

  // Setup the recieve control register (RCTL).
  uint rctl_reg = 0x0;
  rctl_reg |= (1 << 1);  // Reciever enable.
  rctl_reg |= (1 << 2);  // Store bad packets.
  rctl_reg |= (1 << 3);  // Recieve all unicast packets.
  rctl_reg |= (1 << 4);  // Recieve all multicast packets.
  rctl_reg |= (1 << 5);  // Recieve long packets.
  rctl_reg |= (1 << 15); // Accept broadcast packets.
  rctl_reg |= (3 << 16); // Buffer size (4096 bytes).
  rctl_reg |= (1 << 25); // Buffer size extension.
  write_reg(RCTL, rctl_reg);
}

// Transmission initialization.
//
// Reference: Manual - Section 14.5
//
// - Allocate a buffer to hold transmission descriptors.
// - Initialize the transmit descriptor buffer registers.
// - Setup the transmission control register.
// - Setup the transmission inter-packet gap register.
void init_tx() {
  // Transmit buffer should be 16B aligned. Its page aligned,
  // so this is fine
  e1000.tx_buf = kalloc();
  if (e1000.tx_buf == 0) {
    panic("failed to allocate transmission buffer\n");
  }
  memset(e1000.tx_buf, 0, 1 << 12);

  // Setup the transmit descriptor buffer registers.
  write_reg(TDBAL, V2P(e1000.tx_buf));
  write_reg(TDBAH, 0x0);
  write_reg(TDLEN, 1 << 12);
  write_reg(TDH, 0);
  write_reg(TDT, 0);

  // Setup the transmission control TCTL register.
  uint tctl_reg = 0x0;
  tctl_reg |= (1 << 1);
  tctl_reg |= (1 << 3);
  tctl_reg |= ((0xF) << 4);    // Collision threshold.
  tctl_reg |= ((0x200) << 12); // Collision distance.
  write_reg(TCTL, tctl_reg);

  // Setup the transmission inter-packet gap (TIPG) register.
  write_reg(TIPG, 0xA);
}

// Initialize interrupts.
void init_intr() {
  // Enable transmit descriptor write-back and recieve timer interrupts.
  write_reg(IMS,
            (1 << 0) | (1 << 2) | (1 << 3) | (1 << 4) | (1 << 6) | (1 << 7));
}

// Main interrupt handler.
void e1000_intr() {
  // Read the interrupt register to clear interrupts.
  read_reg(ICR);
}

int sys_lspci(void) {
  init();
  init_rx();
  init_tx();
  init_intr();
  ioapicenable(IRQ_PCI0, 0);
  return 0;
}
