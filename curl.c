#include "types.h"
#include "user.h"

int main(int argc, char *argv[]) {
  netopen(0x0A000001, 80, 0);

  // printf(1, "writing data\n");
  // const int n_bytes = netwrite("hello, world", 11);
  // printf(1, "wrote %d bytes\n", n_bytes);

  // netclose(fd);

  exit();
}
