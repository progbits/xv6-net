#include "types.h"
#include "user.h"

int main(int argc, char *argv[]) {
  int netfd = netopen(0x0A000001, 80, 0);
  if (netfd == -1) {
    exit();
  }
  printf(1, "opened netfd\n");

  netwrite(netfd, "hello, world", 12);

  exit();
}
