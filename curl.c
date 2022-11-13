#include "types.h"
#include "user.h"

int main(int argc, char *argv[]) {
  printf(1, "opening connection\n");

  const int fd = netopen("127.0.0.1", 0);
  printf(1, "opened netfd %d\n", fd);

  printf(1, "writing data\n");
  const int n_bytes = netwrite("hello, world", 11);
  printf(1, "wrote %d bytes\n", n_bytes);

  netclose(fd);

  exit();
}
