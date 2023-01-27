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
    printf(2, "usage: nc [destination] [port]\n");
    exit();
  }

  uint addr = parse_addr(argv[1]);
  int port = atoi(argv[2]);

  // Open a new socket for the specified address and port.
  int fd = socket(0);
  bind();
  connect(fd, addr, port);

  // We don't have any `poll` or `select` like functionality, so fork a new
  // process to read received data.
  const uint buf_size = 1024;
  char *buf = malloc(buf_size);
  for (;;) {
    int bytes_read = read(1, buf, buf_size);
    send(fd, buf, bytes_read);
  }

  shutdown(fd);
  exit();
}
