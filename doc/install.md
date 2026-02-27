# Installation

This page outlines how to install `pixivdwn` on your system, and setup the database.

## Dependencies

You'll need to have SQLite installed on your system. On Debian-based systems, you can install it using:

```bash
sudo apt-get install libsqlite3-0
```

## Install binary

`pixivdwn` is written in Rust. There is two way of installing: download the pre-built binary from GitHub, or install from source using Cargo.

- The prebuilt binary is available on [GitHub Releases page](https://github.com/CircuitCoder/pixivdwn/releases). Download and place it in a directory in your PATH
- To install from source, the easiest way is to use `cargo install`. Install the [Rust toolchain](https://rustup.rs/), then:

    ```bash
    cargo install pixivdwn
    ```

    Alternatively, you can clone the repository and build it yourself with `cargo build --release`.