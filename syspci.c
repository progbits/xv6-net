//
// PCI system calls
//

#include "types.h"
#include "x86.h"
#include "defs.h"

const short PCI_CONFIG_ADDR = 0xCF8;
const short PCI_CONFIG_DATA = 0xCFC;

// Find the memory mapped i/o base address of the attached Intel 8254x family
// card. It is assumed that the memory mapped address is held in the first BAR
// register.
unsigned long find_mmio_base() {
  const ushort VENDOR_ID = 0x8086; // Intel
  const ushort DEVICE_ID = 0x100E; // 82540EM Gigabit Ethernet Controller

  // Because we tightly control the environment, assume that the ethernet
  // controller is on one of the first 4 pci devices on the first bus.
  int target_dev = -1; // Set when the target device is found.
  for (int dev = 0; dev < 4; dev++) {
    const unsigned long addr = 0x80000000 | dev << 11;

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
    return 0;
  }

  // Assume the address we want is in the first BAR register.
  unsigned long mmio_addr = 0x0;
  for (int i = 19, j = 3; i >= 16; i--, j--) {
    outdw(PCI_CONFIG_ADDR, (0x80000000 | target_dev << 11) | i);
    uchar data = inb(PCI_CONFIG_DATA);
    mmio_addr = mmio_addr | (data << (j * 8));
  }
  return mmio_addr;
}

// Read the device MAC address from EEPROM.
void read_mac_addr(long mmio_base) {
  const ushort EECD = 0x14; // EEPROM control/data.
  const ushort EERD = 0x14; // EEPROM read.
}

int sys_lspci(void) {
  const long mmio_base = find_mmio_base();
  if (mmio_base == 0) {
    panic("failed to determine base address");
  }
  cprintf("mmio base address: 0x%x\n", mmio_base);
  return 0;
}
