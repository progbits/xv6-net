//
// Network-system system calls.
//
// Single interface with a fixed IP address (10.0.0.2).
//
// TODO:
//     - Split this into sysnet.c and net.c
//     - ARP cache
#include "types.h"
#include "defs.h"
#include "e1000.h"
#include "spinlock.h"

#define NCONN 100

#define ETH_TYPE_IPV4 0x0800
#define ETH_TYPE_IPV6 0x86DD
#define ETH_TYPE_ARP 0x0806

extern struct e1000 e1000;

static struct spinlock netlock;

const ushort fixed_ip[2] = {0x0A00, 0x0002};

// Represents an ethernet frame header.
struct eth {
  ushort dst[3];
  ushort src[3];
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

// Represents an IPV4 packet header.
struct ipv4 {
  uint ihl : 4;
  uint version : 4;
  uchar tos;
  ushort total_len;
  ushort id;
  ushort frag_off;
  uchar ttl;
  uchar protocol;
  ushort check;
  uint src;
  uint dst;
};

// Represents a UDP header.
struct udp {
  ushort src_port;
  ushort dst_port;
  ushort len;
  ushort checksum;
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
  char dst_mac_valid;
};

// ------
// DEBUG.
// ------

void dump_eth_hdr(struct eth *hdr) {
  cprintf("dest mac: 0x");
  for (uint i = 0; i < 3; i++) {
    cprintf("%x", hdr->dst[i] & 0xFFFF);
  }
  cprintf("\n");

  cprintf("source mac: 0x");
  for (uint i = 0; i < 3; i++) {
    cprintf("%x", hdr->src[i] & 0xFFFF);
  }
  cprintf("\n");

  cprintf("ether type: 0x%x\n", hdr->ether_type);
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

// ----------
// End DEBUG.
// ----------

void handle_arp(struct arp_packet *);
void arp_req(uint);

// Active connections.
struct conn conns[NCONN];

ushort __ushort_to_le(ushort value) {
  return (value << 8) | ((value >> 8) & 0xFF);
}

ushort __ushort_to_be(ushort value) {
  ushort tmp = value >> 8;
  return (value << 8) | (tmp & 0xFF);
}

uint __uint_to_le(uint value) {
  uint a = (uint)__ushort_to_le(value >> 16);
  uint b = (uint)__ushort_to_le(value & 0xFFFF);
  return (b << 16) | a;
}

uint __uint_to_be(uint value) {
  uint a = (uint)__ushort_to_be(value & 0xFFFF);
  uint b = (uint)__ushort_to_be(value >> 16);
  return (a << 16) | b;
}

uint eth_from_buf(struct eth *hdr, char *buffer) {
  memmove(hdr, buffer, sizeof(struct eth));
  hdr->ether_type = __ushort_to_le(hdr->ether_type);
  return sizeof(struct eth);
}

// Write an Ethernet frame header a buffer.
uint eth_to_buf(struct eth *hdr, char *buf) {
  struct eth wire;
  memmove(wire.src, hdr->src, 6);
  memmove(wire.dst, hdr->dst, 6);
  wire.ether_type = __ushort_to_be(hdr->ether_type);
  memmove(buf, &wire, sizeof(struct eth));
  return sizeof(struct eth);
}

uint arp_packet_from_buf(struct arp_packet *packet, char *buffer) {
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
  return sizeof(struct arp_packet);
}

// Write an ARP packet to a buffer.
uint arp_packet_to_buf(struct arp_packet *packet, char *buf) {
  struct arp_packet wire;
  wire.htype = __ushort_to_be(packet->htype);
  wire.ptype = __ushort_to_be(packet->ptype);
  wire.hlen = packet->hlen;
  wire.plen = packet->plen;
  wire.oper = __ushort_to_be(packet->oper);
  memmove(wire.sha, packet->sha, 6);
  memmove(wire.tha, packet->tha, 6);
  for (int i = 0; i < 2; i++) {
    wire.spa[i] = __ushort_to_be(packet->spa[i]);
    wire.tpa[i] = __ushort_to_be(packet->tpa[i]);
  }
  memmove(buf, &wire, sizeof(struct arp_packet));
  return sizeof(struct arp_packet);
}

// Write an IPV4 packet to a buffer.
uint ipv4_to_buf(struct ipv4 *seg, char *buf) {
  struct ipv4 wire = {.ihl = seg->ihl,
                      .version = seg->version,
                      .tos = seg->tos,
                      .frag_off = seg->frag_off,
                      .ttl = seg->ttl,
                      .protocol = seg->protocol,
                      .check = seg->check};
  wire.total_len = __ushort_to_be(seg->total_len);
  wire.id = __uint_to_be(seg->id);
  wire.src = __uint_to_be(seg->src);
  wire.dst = __uint_to_be(seg->dst);
  memmove(buf, &wire, sizeof(struct ipv4));
  return sizeof(struct ipv4);
}

// Write a UDP packet to a buffer.
uint udp_to_buf(struct udp *packet, char *buf) {
  struct udp wire;
  wire.src_port = __ushort_to_be(packet->src_port);
  wire.dst_port = __ushort_to_be(packet->dst_port);
  wire.len = __ushort_to_be(packet->len);
  wire.checksum = __ushort_to_be(packet->checksum);
  memmove(buf, &wire, sizeof(struct udp));
  return sizeof(struct udp);
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
int sys_netopen(void) {
  int addr;
  int port;
  int type;
  if (argint(0, &addr) < 0 || argint(1, &port) < 0 || argint(0, &type) < 0) {
    return -1;
  }

  // TODO - Move this to a sysnetinit method.
  initlock(&netlock, "net");

  // Find the first free connection slot.
  int netfd = next_free_netfd();
  if (netfd == -1) {
    return -1;
  }
  conns[netfd].netfd = netfd;
  conns[netfd].src_port =
      netfd; // Map source port 1-1 with network file descriptor.
  memmove(&conns[netfd].dst_addr, &addr, 4);
  conns[netfd].dst_port = port;
  conns[netfd].dst_mac_valid = 0;

  // Resolve the destination MAC address. We do this once per
  // connection, assuming the response will be valid for the duration
  // of the connections lifetime.
  arp_req(addr);

  // Block waiting for ARP response.
  acquire(&netlock);
  while (!conns[netfd].dst_mac_valid) {
    sleep(&conns[netfd], &netlock);
  }
  release(&netlock);

  // Return the netfd used to identify this connection.
  return netfd;
}

int sys_netclose(int netfd) {
  // Remove the connection from the connection list and clean up any resources.
  return -1;
}

// Assume UDP for now and that buf fits in a single frame.
// sys_netwrite(uint netfd, char *buf, uint size)
int sys_netwrite() {
  int netfd;
  char *data;
  int size;
  if (argint(0, &netfd) < 0 || argptr(1, (void *)&data, sizeof(char)) < 0 ||
      argint(2, &size) < 0) {
    return -1;
  }

  // TODO - Check net file descriptor is valid.
  struct conn conn = conns[netfd];

  // Allocate a buffer to hold the outgoing frame.
  char *buf = kalloc();
  if (buf == 0) {
    panic("failed to allocate buffer\n");
  }
  uint offset = 0;

  // Build the ethernet frame and copy it to the buffer.
  struct eth frame = {.ether_type = ETH_TYPE_IPV4};
  memmove(frame.dst, conns[netfd].dst_mac, 6);
  memmove(frame.src, e1000.mac, 6);
  offset += eth_to_buf(&frame, buf + offset);

  // Build the IP header and copy it to the buffer.
  struct ipv4 ipv4 = {.version = 4,
                      .ihl = 5,
                      .total_len = size + 20,
                      .protocol = 0x11,
                      .src = conn.src_addr,
                      .dst = conn.dst_addr};
  offset += ipv4_to_buf(&ipv4, buf + offset);

  // Bulid the UPD packet and copy it to the buffer.
  struct udp packet = {
      .src_port = conn.src_port, .dst_port = conn.dst_port, .len = size};
  offset += udp_to_buf(&packet, buf + offset);

  // Copy the data to the buffer.
  memmove((buf + offset), data, size);
  offset += size;

  // Write the request and release the buffer.
  e1000_write(buf, offset);
  kfree(buf);

  return 1;
}

// Main entrypoint for handling packet rx.
int handle_packet(char *buf, uint size, int end_of_packet) {
  // Read the ethernet header.
  uint offset = 0;
  struct eth hdr;
  offset += eth_from_buf(&hdr, buf);

  switch (hdr.ether_type) {
  case ETH_TYPE_IPV4: {
    // cprintf("ETH_TYPE_IPV4\n");
    break;
  }
  case ETH_TYPE_IPV6: {
    // cprintf("ETH_TYPE_IPV6\n");
    break;
  }
  case ETH_TYPE_ARP: {
    struct arp_packet packet;
    offset += arp_packet_from_buf(&packet, buf + offset);
    // dump_arp_packet(&packet);
    handle_arp(&packet);
    break;
  }
  default: {
    // cprintf("ETH_TYPE_UNKNOWN\n");
    break;
  }
  }
  // dump_eth_hdr(&hdr);

  return 1;
}

// Handle an incoming ARP packet.
void handle_arp(struct arp_packet *req) {
  // Ignore the packet if it doesn't match our address.
  if (req->tpa[0] != fixed_ip[0] || req->tpa[1] != fixed_ip[1]) {
    return;
  }

  // Handle ARP response.
  if (req->oper == 0x2) {
    // Find the connection related to this response, update its connection
    // details and wakeup the process waiting on the response.
    uint i = 0;
    for (; i < NCONN; i++) {
      if (req->spa[0] == ((conns[i].dst_addr >> 16) & 0xFFFF)) {
        if (req->spa[1] == (conns[i].dst_addr & 0xFFFF)) {
          memmove(conns[i].dst_mac, req->sha, 6);
          conns[i].dst_mac_valid = 1;
          wakeup(&conns[i]);
          break;
        }
      }
    }
    return;
  }

  // Handle ARP requests.

  // Allocate a buffer to hold the outgoing frame.
  char *buf = kalloc();
  if (buf == 0) {
    panic("failed to allocate buffer\n");
  }
  uint offset = 0;

  // Build the ethernet frame and copy it to the buffer.
  struct eth frame = {.ether_type = ETH_TYPE_ARP};
  memmove(frame.src, e1000.mac, 6);
  memmove(frame.dst, req->sha, 6);
  offset += eth_to_buf(&frame, buf);

  // Bulid the ARP response and copy it to the buffer.
  struct arp_packet res = {
      .htype = 0x1, .ptype = 0x0800, .hlen = 0x6, .plen = 0x4, .oper = 0x2};
  memmove(res.sha, e1000.mac, 6);
  memmove(res.tha, req->spa, 6);
  memmove(res.spa, fixed_ip, 4);
  memmove(res.tpa, req->spa, 4);
  offset += arp_packet_to_buf(&res, buf + offset);

  // Write the response and release the buffer.
  e1000_write(buf, offset);
  kfree(buf);
}

// Make an ARP request for a given address.
void arp_req(uint addr) {
  // Allocate a buffer to hold the outgoing frame.
  char *buf = kalloc();
  if (buf == 0) {
    panic("failed to allocate buffer\n");
  }
  uint offset = 0;

  // Build the ethernet frame and copy it to the buffer.
  struct eth frame = {.dst = {0xFFFF, 0xFFFF, 0xFFFF},
                      .ether_type = ETH_TYPE_ARP};
  memmove(frame.src, e1000.mac, 6);
  offset += eth_to_buf(&frame, buf);

  // Bulid the ARP request and copy it to the buffer.
  struct arp_packet res = {.htype = 0x1,
                           .ptype = 0x0800,
                           .hlen = 0x6,
                           .plen = 0x4,
                           .oper = 0x1,
                           .tha = {0xFFFF, 0xFFFF, 0xFFFF}};
  memmove(res.sha, e1000.mac, 6);
  memmove(res.spa, fixed_ip, 4);
  ushort addr_be[2] = {(addr >> 16) & 0xFFFF, addr & 0xFFFF};
  memmove(res.tpa, &addr_be, 4);
  offset += arp_packet_to_buf(&res, buf + offset);

  // Write the request and release the buffer.
  e1000_write(buf, offset);
  kfree(buf);
}
