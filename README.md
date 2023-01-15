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
