//
// Network-system system calls.
//
// Single interface with a fixed IP address (10.0.0.2).
//
// TODO - Split this into sysnet.c and net.c
//

#include "types.h"
#include "defs.h"
#include "e1000.h"

#define NCONN 100

#define ETH_TYPE_IPV4 0x0800
#define ETH_TYPE_IPV6 0x86DD
#define ETH_TYPE_ARP 0x0806

extern struct e1000 e1000;

const ushort fixed_ip[2] = {0x0A00, 0x0002};

// Represents an ethernet frame header.
struct eth {
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

// Represents a UDP/TCP connection.
struct conn {
  int netfd;
  uint type;
  uint src_addr;
  uint src_port;
  uint dst_addr;
  uint dst_port;
  char dst_mac[6];
};

void handle_arp_req(struct arp_packet *);
void make_arp_req(uint);

// Active connections.
struct conn conns[NCONN];

ushort __ushort_to_le(ushort value) {
  return (value << 8) | ((value >> 8) & 0xFF);
}

void eth_from_buf(struct eth *hdr, char *buffer) {
  memmove(hdr, buffer, sizeof(struct eth));
  hdr->ether_type = __ushort_to_le(hdr->ether_type);
}

void dump_eth_hdr(struct eth *hdr) {
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
  memmove(packet, buffer, sizeof(struct arp_packet));
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

// Return the next free network file descriptor.
int next_free_netfd() {
  for (uint i = 0; i < NCONN; i++) {
    if (conns[i].netfd == 0) {
      return i;
    }
  }
  return -1;
}

// Release a netfd.
int free_netfd(int netfd) {
  for (uint i = 0; i < NCONN; i++) {
    if (conns[i].netfd == netfd) {
      conns[i].netfd = 0;
      return 1;
    }
  }
  return -1;
}

// Open a new network connection.
//
// TODO - We don't have a seperate open(), connect()/accept() flow, so specify
// direction at creation time?. Specify port separately, for outbound
// connections interpret as dst_port, for inbound connections, src_port.
int sys_netopen(uint addr, uint port, uint type) {
  // Find the first free connection slot.
  int netfd = next_free_netfd();
  if (netfd == -1) {
    return -1;
  }
  conns[netfd].netfd = netfd;
  conns[netfd].src_port = netfd; // Map source port 1-1 with fd.

  // Resolve the destination MAC address. We do this once per connection, on
  // open.
  // TODO - Global ARP cache.
  make_arp_req(addr);

  // Return the netfd used to identify this connection.
  return netfd;
}

int sys_netclose(int netfd) {
  // Remove the connection from the connection list and clean up any resources.
  return -1;
}

int sys_netwrite(char *buf, uint size) { return -1; }

// Main entrypoint for handling packet rx.
int handle_packet(char *buf, uint size, int end_of_packet) {
  // Read the ethernet header.
  uint offset = 0;
  struct eth hdr;
  eth_from_buf(&hdr, buf);
  offset += sizeof(struct eth);
  switch (hdr.ether_type) {
  case ETH_TYPE_IPV4: {
    cprintf("ETH_TYPE_IPV4\n");
    break;
  }
  case ETH_TYPE_IPV6: {
    cprintf("ETH_TYPE_IPV6\n");
    break;
  }
  case ETH_TYPE_ARP: {
    cprintf("ETH_TYPE_ARP\n");
    struct arp_packet packet;
    arp_packet_from_buf(&packet, buf + offset);
    dump_arp_packet(&packet);
    offset += sizeof(struct arp_packet);
    handle_arp_req(&packet);
    break;
  }
  default: {
    cprintf("ETH_TYPE_UNKNOWN\n");
    break;
  }
  }
  dump_eth_hdr(&hdr);

  return 1;
}

// Handle an incoming ARP request.
void handle_arp_req(struct arp_packet *packet) {
  cprintf("HANDLE ARP REQUEST\n");
}

// Make an ARP request for a given address.
void make_arp_req(uint addr) {
  struct arp_packet req = {
      .htype = 0x1, .ptype = 0x0800, .hlen = 0x6, .plen = 0x4, .oper = 0x2};

  // Send from our MAC.
  // TODO - Better way to get this than reaching "down" into the e1000 struct?
  for (int i = 0; i < 3; i++) {
    const ushort first = e1000.mac[i * 2] & 0xFF;
    const ushort second = e1000.mac[i * 2 + 1] & 0xFF;
    req.sha[i] = (first << 8) | second;
    req.tha[i] = addr;
  }

  // Request is sent from out fixed IP.
  for (int i = 0; i < 2; i++) {
    req.spa[i] = fixed_ip[i];
    req.tpa[i] = addr >> (i * 16) & 0xFFFF;
  }

  e1000_write((char *)&req, sizeof(struct arp_packet));
}
