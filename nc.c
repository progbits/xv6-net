#include "types.h"
#include "user.h"

// Parse the hexadecimal representation of an IPV4 address from its 'dot'
// representation.
uint parse_addr(char *addr) {
  uint target_addr = 0x0;
  uint shift = 24;
  uint addr_len = strlen(addr);
  for (int i = 0; i < addr_len; i++) {
    char part[4] = {0x0, 0x0, 0x0, 0x0};
    int sep = i;
    for (int j = i; j < addr_len; j++) {
      if (addr[j] == '.') {
        sep = j;
        break;
      }
      sep = j + 1; // No separator after last octet.
    }
    memmove(&part, addr + i, sep - i);

    int octet = atoi(part);
    target_addr |= octet << (shift);
    shift -= 8;

    i = sep;
  }
  return target_addr;
}

// A simple `nc` like program. As we have a UDP only network stack, the `-l`
// flag is somewhat implicit.
int main(int argc, char *argv[]) {
  if (argc < 3) {
    printf(2, "usage: nc [destination] [port]");
    exit();
  }

  uint addr = parse_addr(argv[1]);
  int port = atoi(argv[2]);

  // Open a new network file descriptor for the specified address and port.
  int netfd = netopen(addr, port, 0);
  if (netfd == -1) {
    exit();
  }

  // Write a bunch of data.
  char *outputs[] = {"hello\n", "test\n", "foo\n", "bar\n", "!\n"};
  for (int i = 0; i < 2048; i++) {
    for (int j = 0; j < 5; j++) {
      netwrite(netfd, outputs[j], strlen(outputs[j]));
    }
  }
  netclose(netfd);

  exit();
}
