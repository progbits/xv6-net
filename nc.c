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
  int fd = socket(0);
  bind();
  connect(fd, 0x0A000001, 5432);
  //listen();
  //accept();
  send(fd, "hello, world\n", 14);
  //recv();
  shutdown(fd);

  printf(2, "socketfd %d\n", fd);

  if (argc < 3) {
    printf(2, "usage: nc [destination] [port]\n");
    exit();
  }

  uint addr = parse_addr(argv[1]);
  int port = atoi(argv[2]);

  // Open a new network file descriptor for the specified address and port.
  int netfd = netopen(addr, port, 0);
  if (netfd == -1) {
    exit();
  }

  // We don't have any `poll` or `select` like functionality, so fork a new
  // process to read received data.
  int pid = fork();
  if (pid == 0) {
    // Forked process
    const uint buf_size = 2048; // Big enough to hold any packet.
    char *buf = malloc(buf_size);
    for (;;) {
      int bytes_read = netread(netfd, buf, buf_size);
      buf[bytes_read] = '\0';
      printf(1, "%s", buf);
    }
  } else {
    // Original process.
    const uint buf_size = 1024;
    char *buf = malloc(buf_size);
    for (;;) {
      int bytes_read = read(1, buf, buf_size);
      netwrite(netfd, buf, bytes_read);
    }
  }

  netclose(netfd);
  exit();
}
