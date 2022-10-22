//
// PCI system calls
//

#include "types.h"
#include "x86.h"
#include "defs.h"

int sys_lspci(void) {
  const short PCI_CONFIG_ADDR = 0xCF8;
  const short PCI_CONFIG_DATA = 0xCFC;

  // Enumerate the first 4 pci buses.
  for (int bus = 0; bus < 4; bus++) {
    cprintf("pci_bus: %d\n", bus);
    long addr = 0x80000000 | bus << 11;

    cprintf("vendor_id: 0x");
    for (int i = 1; i >= 0; i--) {
      outdw(PCI_CONFIG_ADDR, addr | i);
      uchar data = inb(PCI_CONFIG_DATA);
      cprintf("%x", data);
    }
    cprintf("\n");

    cprintf("device_id: 0x");
    for (int i = 3; i >= 2; i--) {
      outdw(PCI_CONFIG_ADDR, addr | i);
      uchar data = inb(PCI_CONFIG_DATA);
      cprintf("%x", data);
    }
    cprintf("\n");
  }
  return 0;
}
