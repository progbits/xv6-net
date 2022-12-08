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

#define PORT_OFFSET 3000

#define ETH_TYPE_IPV4 0x0800
#define ETH_TYPE_IPV6 0x86DD
#define ETH_TYPE_ARP 0x0806

// The fixed address of our single adaptor.
const ushort fixed_ip[2] = {0x0A00, 0x0002};
const uint fixedip = 0x0A000002;

extern struct e1000 e1000;

// Spinlock protecting all shared network resources.
static struct spinlock netlock;

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
// Note: The source address is implicit in the fixed address of the default
// adaptor.
struct conn {
  int netfd;
  uint type;
  uint src_port;
  uint dst_addr;
  uint dst_port;
  char dst_mac[6];
  char dst_mac_valid;
  char *buf;
  uint size; // The number of bytes waiting to be read from the connection.
};

void handle_arp(struct arp_packet *);
void arp_req(uint);
void handle_udp(struct udp *, char *);

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
  uint b = (uint)__ushort_to_be((value >> 16) & 0xFFFF);
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
  for (uint i = 0; i < 3; i++) {
    wire.dst[i] = __ushort_to_be(hdr->dst[i]);
  }
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

// Read an IPV4 header from a buffer.
uint ipv4_from_buf(struct ipv4 *header, char *buf) {
  memmove(header, buf, sizeof(struct ipv4));
  header->total_len = __ushort_to_le(header->total_len);
  header->id = __ushort_to_le(header->id);
  header->src = __uint_to_le(header->src);
  header->dst = __uint_to_le(header->dst);
  return sizeof(struct ipv4);
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

// Read a UDP packet from a buffer.
uint udp_from_buf(struct udp *packet, char *buf) {
  memmove(packet, buf, sizeof(struct udp));
  packet->src_port = __ushort_to_le(packet->src_port);
  packet->dst_port = __ushort_to_le(packet->dst_port);
  packet->len = __ushort_to_le(packet->len);
  return sizeof(struct udp);
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
    if (conns[i].netfd == -1) {
      return i;
    }
  }
  return -1;
}

void netinit() {
  initlock(&netlock, "net");
  for (int i = 0; i < NCONN; i++) {
    conns[i].netfd = -1;
  }
}

// Open a new client network connection.
//
// Opening a network connection establishes an association with a
// local (address, port) tuple and the calling process. For connection
// oriented sockets, this method will also attempt to establish a
// connection. Equivalent a combination of the Berkley socket
// interface socket(), bind() and connect() methods.
//
// As we only support a single network interface with a hardcoded
// address, so we expose no concept of binding to a local address or
// adaptor. All network connections are bound explicitly to the default
// adaptor and fixed network address.
//
// addr - The address of the remote host associated with the
// connection. For connectionless protocols (UDP), this method
// establishes the destination for subsequent calls to `netwrite`. For
// connection-oriented protocols (TCP), this method attempts to
// establish a new connection.
//
// port - The port of the remote host associated with the
// connection.
//
// type - The type of connection, either 0 (UDP) or 1 (TCP).
int sys_netopen(void) {
  int addr;
  int port;
  int type;
  if ((argint(0, &addr) < 0) || (argint(1, &port) < 0) ||
      (argint(0, &type) < 0)) {
    return -1;
  }

  // Find the first free connection slot.
  int netfd = next_free_netfd();
  if (netfd == -1) {
    return -1;
  }
  conns[netfd].netfd = netfd;
  conns[netfd].src_port =
      netfd + PORT_OFFSET; // Map source port as file descriptor + offset.
  memmove(&conns[netfd].dst_addr, &addr, 4);
  conns[netfd].dst_port = port;
  conns[netfd].dst_mac_valid = 0;
  conns[netfd].buf = kalloc();
  if (conns[netfd].buf == 0) {
    panic("failed to allocate buffer\n");
  }
  conns[netfd].size = 0;

  // Resolve the destination MAC address. We do this once per
  // connection, assuming the response will be valid for the duration
  // of the connections lifetime.
  arp_req(addr);

  // Block waiting for ARP response.
  // TODO: Timeouts
  acquire(&netlock);
  while (!conns[netfd].dst_mac_valid) {
    sleep(&conns[netfd], &netlock);
  }
  release(&netlock);

  // Return the netfd used to identify this connection.
  return netfd;
}

// Release a netfd and free any associated resources.
int sys_netclose() {
  int netfd;
  if (argint(0, &netfd) < 0) {
    return -1;
  }

  if (conns[netfd].netfd == -1) {
    // Already free.
    return 0;
  }
  kfree(conns[netfd].buf);

  conns[netfd].netfd = -1;
  conns[netfd].type = -1;
  conns[netfd].src_port = -1;
  conns[netfd].dst_addr = -1;
  conns[netfd].dst_port = -1;
  memset(conns[netfd].dst_mac, 0, 6);
  conns[netfd].dst_mac_valid = 0;
  conns[netfd].buf = 0;
  conns[netfd].size = 0;

  return 0;
}

// Write a new UDP segment to a netfd.
int sys_netwrite() {
  int netfd;
  char *data;
  int size;
  if (argint(0, &netfd) < 0 || argptr(1, (void *)&data, sizeof(char)) < 0 ||
      argint(2, &size) < 0) {
    return -1;
  }

  // TODO - Check net file descriptor is valid.
  acquire(&netlock);
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
  const uint total_len = sizeof(struct ipv4) + sizeof(struct udp) + size;
  struct ipv4 ipv4 = {.version = 4,
                      .ihl = 5,
                      .total_len = total_len,
                      .protocol = 0x11,
                      .ttl = 64,
                      .src = fixedip,
                      .dst = conn.dst_addr};
  offset += ipv4_to_buf(&ipv4, buf + offset);

  // Build the UPD packet and copy it to the buffer.
  struct udp packet = {.src_port = conn.src_port,
                       .dst_port = conn.dst_port,
                       .len = sizeof(struct udp) + size};
  offset += udp_to_buf(&packet, buf + offset);

  // Copy the data to the buffer.
  memmove((buf + offset), data, size);
  offset += size;

  // Write the request and release the buffer.
  e1000_write(buf, offset, 1);
  kfree(buf);

  release(&netlock);
  return 0;
}

int sys_netread() {
  int netfd;
  char *data;
  int size;
  if ((argint(0, &netfd) < 0) || (argptr(1, (void *)&data, sizeof(char)) < 0) ||
      (argint(2, &size) < 0)) {
    return -1;
  }

  // Block waiting for data.
  acquire(&netlock);
  while (!conns[netfd].size) {
    sleep(&conns[netfd], &netlock);
  }

  // Copy data into userspace buffer.
  uint to_copy = conns[netfd].size < size ? conns[netfd].size : size;
  memmove(data, conns[netfd].buf, to_copy);

  conns[netfd].size -= to_copy;

  // Return number of bytes read.
  release(&netlock);
  return to_copy;
}

// Main entrypoint for handling packet rx.
int handle_packet(char *buf, int size, int end_of_packet) {
  acquire(&netlock);

  // Read the ethernet header. Assume that MAC filtering is happening in
  // hardware. We are only receiving packets destined for us.
  uint offset = 0; // The offset into the packet buffer.
  struct eth hdr;
  offset += eth_from_buf(&hdr, buf);

  switch (hdr.ether_type) {
  case ETH_TYPE_IPV4: {
    struct ipv4 header;
    offset += ipv4_from_buf(&header, buf + offset);

    // Drop any packets that aren't for us.
    if (header.dst != fixedip) {
      goto end;
    }

    // Handle the packet.
    if (header.protocol == 0x11) {
      struct udp udp;
      offset += udp_from_buf(&udp, buf + offset);
      handle_udp(&udp, buf + offset);
    }
    break;
  }
  case ETH_TYPE_IPV6: {
    break;
  }
  case ETH_TYPE_ARP: {
    struct arp_packet packet;
    offset += arp_packet_from_buf(&packet, buf + offset);
    handle_arp(&packet);
    break;
  }
  default: {
    break;
  }
  }

end:
  release(&netlock);
  return 0;
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

  // Build the ARP response and copy it to the buffer.
  struct arp_packet res = {
      .htype = 0x1, .ptype = 0x0800, .hlen = 0x6, .plen = 0x4, .oper = 0x2};
  memmove(res.sha, e1000.mac, 6);
  memmove(res.tha, req->spa, 6);
  memmove(res.spa, fixed_ip, 4);
  memmove(res.tpa, req->spa, 4);
  offset += arp_packet_to_buf(&res, buf + offset);

  // Write the response and release the buffer.
  e1000_write(buf, offset, 0);
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

  // Build the ARP request and copy it to the buffer.
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
  e1000_write(buf, offset, 0);
  kfree(buf);
}

// Handle an incoming UDP packet.
void handle_udp(struct udp *packet, char *buf) {
  // Find the netfd related to this response, copy packet data into its buffer
  // and wakeup the process waiting on the response.
  for (uint i = 0; i < NCONN; i++) {
    if ((conns[i].src_port) == packet->dst_port) {
      uint data_len = packet->len - sizeof(struct udp);
      memmove(conns[i].buf + conns[i].size, buf, data_len);
      conns[i].size += data_len;
      wakeup(&conns[i]);
      break;
    }
  }
}
