#include "types.h"
#include "user.h"

enum mode { MODE_SEND, MODE_LISTEN, MODE_UNKNOWN };

const char *usage = "usage: nc [-c|-s] [address] [port]\n";

enum mode parse_mode(char *mode) {
  if (mode[1] == 'c') {
    return MODE_SEND;
  } else if (mode[1] == 's') {
    return MODE_LISTEN;
  }
  return MODE_UNKNOWN;
}

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
    printf(2, usage);
    exit();
  }

  // Are we in 'send' (0) or 'listen' (1) mode?
  enum mode mode = parse_mode(argv[1]);
  if (mode == MODE_UNKNOWN) {
    printf(2, usage);
    exit();
  }

  // Parse the address and port.
  uint addr = parse_addr(argv[2]);
  int port = atoi(argv[3]);

  // Open a new socket and setup the send/receive buffer.
  int sockfd = socket(0);
  const uint buf_size = 1024;
  char *buf = malloc(buf_size);

  if (mode == MODE_SEND) {
    // Set up the socket with details of the remote address and port.
    connect(sockfd, addr, port);
    // Send data from stdin to the socket.
    for (;;) {
      int bytes_read = read(1, buf, buf_size);
      send(sockfd, buf, bytes_read);
    }
  } else if (mode == MODE_LISTEN) {
    // Bind the socket to the specified local address and port.
    bind(sockfd, addr, port);
    // Read data from the socket.
    for (;;) {
      int bytes_read = recv(sockfd, buf, buf_size - 1);
      if (bytes_read > 0) {
        buf[bytes_read] = '\x00';
        printf(1, "%s", buf);
      } else {
        sleep(1);
      }
    }
  }

  shutdown(sockfd);
  exit();
}
