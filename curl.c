#include "types.h"
#include "user.h"

int main(int argc, char *argv[]) {
  int netfd = netopen(0x0A000001, 5000, 0);
  if (netfd == -1) {
    exit();
  }
  printf(1, "opened netfd %d\n", netfd);

  char *outputs[] = {"hello", "test", "foo", "ba", "z"};
  uint lens[] = {5, 4, 3, 2, 1};

  for (uint i = 0; i < 5; i++) {
    netwrite(netfd, outputs[i], lens[i]);
  }

  printf(1, "waiting for message\n");

  uint buf_size = 1 << 12;
  char *buf = malloc(buf_size);
  netread(netfd, buf, buf_size);

  printf(1, "got message %s\n", buf);

  exit();
}
