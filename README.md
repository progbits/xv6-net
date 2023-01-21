# xv6-net

A network stack for the [xv6](https://en.wikipedia.org/wiki/Xv6) operating
system, written in [Rust](https://www.rust-lang.org).

## Getting Started

### Prerequisites

Building and running the `xv6-net` the project requries the following dependencies:

- A C compiler (e.g. GCC)
- Make
- Rust (and the i586-unknown-linux-gnu toolchain)
- QEMU

On Ubuntu-like operating systems, a C compiler (GCC) and Make can be installed
as part of the `build-essential` package:

```shell
sudo apt-get install build-essential
```

Rust can be installed by following the instructions on the Rust project's
[homepage](https://www.rust-lang.org/tools/install). The
`i586-unknown-linux-gnu` toolchain can be installed using the `rustup` tool:

```shell
rustup toolchain install i586-unknown-linux-gnu
```

QEMU can be installed **via** the `qemu` package:

```shell
apt-get install qemu
```

### Running the Project

In addition to the standard Make targets provided by the `xv6` project, the
`xv6-net` project adds a `qemu-net` Make target. This target uses QEMU's `-nic`
[flag](https://wiki.qemu.org/Documentation/Networking) to emulate an E1000
family network device. The `-nic`
[flag](https://wiki.qemu.org/Documentation/Networking) creates a new
[TAP](https://en.wikipedia.org/wiki/TUN/TAP) device which may require root
privileges.

The `xv6` network stack supports a single interface which is assigned a fixed
address of `10.0.0.2`.

The project can be built with `make`:

```shell
make
```

Once built, the `xv6` operating system can be started with the `qmu-net` target:

```shell
make qemu-net
```
After assigning an address to the TAP interface:

```shell
ip addr add 10.0.0.2 dev tap0
```

The `xv6` operating system should respond to ping.

```shell
$ ping 10.0.0.2
PING 10.0.0.2 (10.0.0.2) 56(84) bytes of data.
64 bytes from 10.0.0.2: icmp_seq=1 ttl=64 time=0.026 ms
64 bytes from 10.0.0.2: icmp_seq=2 ttl=64 time=0.035 ms
64 bytes from 10.0.0.2: icmp_seq=3 ttl=64 time=0.021 ms
```
