#ifndef E1000_H
#define E1000_H

struct e1000 {
  uint mmio_base;     // The base address of the cards MMIO region.
  char mac[6];        // The cards EEPROM configured MAC address.
  struct rx_desc *rx; // Page sized buffer holding receive descriptors.
  char **rx_buf;      // List of page sized receive data buffers.
  uint rx_count;      // The number of receive descriptors allocated.
  uint rx_i;          // Index of the next receive descriptor to read.
  struct tx_desc *tx; // Page sized buffer holding transmit descriptors.
  char **tx_buf;      // List of page sized transmit data buffers.
  uint tx_count;      // Number of transmit descriptors allocated.
  uint tx_i;          // Index of the next transmit buffer to write.
  char tx_ctx;        // Have we setup the transmit context descriptor.
};

int e1000_read();
int e1000_write(const char *buf, int size, int offload);

#endif
