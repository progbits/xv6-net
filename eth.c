#include "eth.h"

ushort __ushort_to_le(ushort value) {
  return (value << 8) | ((value >> 8) & 0xFF);
}

void eth_hdr_from_buf(struct eth_hdr *hdr, char *buffer) {
  memcpy(hdr, buffer, sizeof(struct eth_hdr));
  hdr->ether_type = __ushort_to_le(hdr->ether_type);
}

void dump_eth_hdr(struct eth_hdr *hdr) {
  cprintf("dest mac: 0x");
  for (uint i = 0; i < 6; i++) {
    cprintf("%x", hdr->dest[i] & 0xFF);
  }
  cprintf("\n");

  cprintf("source mac: 0x");
  for (uint i = 0; i < 6; i++) {
    cprintf("%x", hdr->source[i] & 0xFF);
  }
  cprintf("\n");

  cprintf("ether type: 0x%x\n", hdr->ether_type);
}

void arp_packet_from_buf(struct arp_packet *packet, char *buffer) {
  memcpy(packet, buffer, sizeof(struct arp_packet));
  packet->htype = __ushort_to_le(packet->htype);
  packet->ptype = __ushort_to_le(packet->ptype);
  packet->hlen = __ushort_to_le(packet->hlen);
  packet->oper = __ushort_to_le(packet->oper);
  for (uint i = 0; i < 3; i++) {
    packet->sha[i] = __ushort_to_le(packet->sha[i]);
    packet->tha[i] = __ushort_to_le(packet->tha[i]);
  }

  for (uint i = 0; i < 2; i++) {
    packet->spa[i] = __ushort_to_le(packet->spa[i]);
    packet->tpa[i] = __ushort_to_le(packet->tpa[i]);
  }
}

void dump_arp_packet(struct arp_packet *packet) {
  cprintf("hardware type: 0x%x\n", packet->htype);
  cprintf("protocol type: 0x%x\n", packet->ptype);
  cprintf("operation: 0x%x\n", packet->oper);

  cprintf("sender hardware address: 0x");
  for (uint i = 0; i < 3; i++) {
    cprintf("%x", packet->sha[i] & 0xFFFF);
  }
  cprintf("\n");

  cprintf("target protocol address: 0x");
  for (uint i = 0; i < 2; i++) {
    cprintf("%x", packet->tpa[i] & 0xFFFF);
  }
  cprintf("\n");
}
