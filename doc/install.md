# Installation

This page outlines how to install `pixivdwn` on your system, and setup the database.

## Install binary

`pixivdwn` is written in Rust with statically linked sqlite3. There is two way of installing: download the pre-built binary from GitHub, or install from source using Cargo.

- The prebuilt binary is available on [GitHub Releases page](https://github.com/CircuitCoder/pixivdwn/releases). Download and place it in a directory in your PATH
- To install from source, the easiest way is to use `cargo install`. Install the [Rust toolchain](https://rustup.rs/), then:

    ```bash
    cargo install pixivdwn
    ```

    Alternatively, you can clone the repository and build it yourself with `cargo build --release`.

## Setup

After you downloaded the binary, you may want to setup your local files. `pixivdwn` uses environment variables to configure the database URL, Pixiv/Fanbox cookies, and base download directories. The recommended way is to set them through a `.env` file. This means that you might want to choose a dedicated working directory for `pixivdwn` to store the `.env` and database files with appropriate permissions. If we choose `/usr/local/share/pixivdwn`:

```bash
mkdir -p /usr/local/share/pixivdwn
cd /usr/local/share/pixivdwn

echo >> .env << EOF
DATABASE_URL=sqlite://./db.sqlite
PIXIV_COOKIE=<xxxxxxx_xxxxxxxxxxxxxxxxxx>
FANBOX_HEADER_FULL='<THE ENTIRE FANBOX HEADER>
<CAN BE MULTIPLE LINES>'
PIXIV_BASE_DIR=./pixiv
FANBOX_BASE_DIR=./fanbox
EOF

pixivdwn setup
```

If you never use fanbox, you can ignore the fanbox-related environment variables (and vice-versa for pixiv-related variables). You can get the full header of a fanbox request by using your browser's developer tools, open `fanbox.cc`, select any request to `fanbox.cc` domain in the "Network" tab, and copy the entire header as text. In Firefox, this is done by:

- Open dev tools with right click -> "Inspect Element" on the page, or just press `F12`
- Goto "Network" tab. If empty, refresh the page.
- Select any request to `fanbox.cc` domain (the first one should always work), right click -> "Copy Value" -> "Copy Request Headers".
- Paste them into your `.env` file as mentioned above. Yes, you can keep the `GET <path> HTTP/<version>` line at the top.
