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
const uint ICR = 0x000C0;
const uint IMS = 0x000D0;
const uint TIPG = 0x00410;
const uint TDFPC = 0x03430;
const uint TDBAL = 0x03800;
const uint TDBAH = 0x03804;
const uint TDLEN = 0x03808;
const uint TDH = 0x03810;
const uint TDT = 0x03818;
const uint TCTL = 0x00400;
const uint GPTC = 0x04080;
const uint TPT = 0x040D4;
const uint PBM_START = 0x10000;

static struct e1000 {
  uint mmio_base; // The base address of the cards MMIO region.
  char mac[6];    // The cards EEPROM configured MAC address.
  char *txn_buf;  // Page sized buffer holding transmit descriptors.
} e1000;

// Read a main function register.
uint read_reg(uint reg) { return *(uint *)(e1000.mmio_base + reg); }

// Write a main function register.
void write_reg(uint reg, uint value) {
  *(uint *)(e1000.mmio_base + reg) = value;
}

// Locate an attached Intel 8254x family ethernet card and record its
// MMIO base address. It is assumed that the memory mapped address is
// held in the first BAR register.
//
// As we are reading the PCI configuration space, we also ensure the
// card is configured to act as a bus master for DMA.
void detect_e1000() {
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
}

// General initialization.
//
// - Read the device MAC address from EEPROM.
void init() {
  const ushort EERD = 0x14; // EEPROM register offset.
  const uint EEPROM_DONE = 0x00000010;

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

// Transmission initialization.
//
// Reference: Manual - Section 14.5
//
// - Allocate a buffer to hold transmission descriptors.
// - Initialize the transmit descriptor buffer registers.
// - Setup the transmission control register.
// - Setup the transmission inter-packet gap register.
void init_txn() {
  // Transmit buffer should be 16B aligned. Its page aligned,
  // so this is fine
  e1000.txn_buf = kalloc();
  if (e1000.txn_buf == 0) {
    panic("failed to allocate transmission buffer\n");
  }
  memset(e1000.txn_buf, 0, 1 << 12);

  // Setup the transmit descriptor buffer registers.
  write_reg(TDBAL, V2P(e1000.txn_buf));
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
  write_reg(IMS, (1 << 0) | (1 << 7));
}

// Main interrupt handler.
void e1000_intr() {
  // Read the interrupt register to clear interrupts.
  read_reg(ICR);
}

int sys_lspci(void) {
  detect_e1000();
  init();
  init_txn();
  init_intr();
  ioapicenable(IRQ_PCI0, 0);
  return 0;
}
