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
$ make
```

Once built, the `xv6` operating system can be started with the `qemu-net` target:

```shell
$ make qemu-net
```
After assigning an address to the TAP interface:

```shell
$ ip addr add 10.0.0.2 dev tap0
```

The `xv6` operating system should respond to ping.

```shell
$ ping 10.0.0.2
PING 10.0.0.2 (10.0.0.2) 56(84) bytes of data.
64 bytes from 10.0.0.2: icmp_seq=1 ttl=64 time=0.026 ms
64 bytes from 10.0.0.2: icmp_seq=2 ttl=64 time=0.035 ms
64 bytes from 10.0.0.2: icmp_seq=3 ttl=64 time=0.021 ms
```

## New System Calls

The implementation of network stack adds 7 new system calls: `socket`, `bind`,
`connect`, `listen`, `accept`, `send `and `recv`. As only UDP is currently
supported, the `listen` and `accept` system calls are currently nops.

- `socket` - Creates a new socket of the specified type (currently UDP only)
- `bind` - Associates a socket with a local address and port
- `connect` - Associates a socket with a remote address and port
- `listen` - Not implemented
- `accept` - Not implemented
- `send` - Send data to a remote socket
- `recv` - Receive data from a remote socket

## Using the Network Stack

An example netcat like userspace program, `nc`, is provided to exercise the
network stack.

The `nc` program operates in two modes, client or server:

```shell
nc [-c|-s] [address] [port]
```

In client mode (`-c`), the program will send data from stdin to the specified
port of the host located at `address`. For example, to send data to `10.0.0.1` on port `4444`:

```shell
$ nc -c 10.0.0.1 4444
$ hello, world!
```

In server mode (`-s`), the program will listen for data on the specified port.
To listen to data on port `5555`:

```shell
$ nc -s 10.0.0.2 5555
```

As the network interface has a fixed local address (`10.0.0.2`), the `address`
argument is currently ignored in server mode.

## Notes

- The network interface is assigned a fixed address of `10.0.0.2`
- The connect(...) system call is blocking on establishing the ARP resolution
  of the hardware address of the remote host.
- The send(...) system call is blocking on the successful write of a transmit
  descriptor to the network device.
- The recv(...) system call is non-blocking, returning immediately if no data
  is available. This is until the functionality of proc.c is ported to Rust.
