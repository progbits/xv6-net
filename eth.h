#ifndef ETH_H
#define ETH_H

#include "types.h"

// Represents an ethernet frame.
struct eth_hdr {
  uchar dest[6];
  uchar source[6];
  ushort ether_type;
};

// Represents an ARP packet.
struct arp_packet {
  ushort htype;
  ushort ptype;
  uchar hlen;
  uchar plen;
  ushort oper;
  ushort sha[3];
  ushort spa[2];
  ushort tha[3];
  ushort tpa[2];
};

ushort __ushort_to_le(ushort value);

void eth_hdr_from_buf(struct eth_hdr *hdr, char *buffer);

void dump_eth_hdr(struct eth_hdr *hdr);

void arp_packet_from_buf(struct arp_packet *packet, char *buffer);

#endif
