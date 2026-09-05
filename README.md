# libchiaki

Rust bindings for libchiaki — chiaki-ng's core remote-play library (chiaki Session, Discovery, Regist, holepunching, Takion), FFI-generated with bindgen, no C shim.

## Layout

- `build.rs` — links the prebuilt static chiaki stack (`libchiaki.a` + bundled curl/jerasure/gf_complete/protobuf-nanopb), runs bindgen over the installed chiaki headers, and compiles bindgen's `wrap_static_fns` C wrappers.
- `scripts/build-chiaki.sh` — builds the C side into a dev prefix (`$LIBCHIAKI_PREFIX`: `include/` + `lib/`).

## Building the C library

The script takes no positional arguments; everything is configured through
environment variables (`LIBCHIAKI_PREFIX` is required; `CHIAKI_SRC_DIR`,
`CHIAKI_VERSION`, `BUILD_TYPE`, ... are optional, see the header comment in
the script):

```sh
export LIBCHIAKI_PREFIX="$PWD/libchiaki-install"   # required: output prefix
scripts/build-chiaki.sh                            # clones/updates chiaki-ng itself
cargo build
```

Per-platform toolchain expectations:

- **Linux** — system packages for the shared deps (libssl, opus, json-c, libevent, miniupnpc, zlib); located via pkg-config at build time.
- **macOS** — the script builds the deps statically into the prefix; no system packages needed beyond build tools.
- **Windows** — MSYS2 MINGW64; deps come from the mingw64 sysroot and are linked statically (`cargo build --target x86_64-pc-windows-gnu`). MSVC toolchains cannot be used (the prebuilt archives are MinGW/COFF).

## curl configuration requirements (important)

chiaki uses curl only for HTTP/1.1 + WebSocket (holepunching). This crate links
`libcurl.a` assuming every feature that drags in an extra external library is
disabled. If you build libchiaki yourself, configure curl with at least:

```cmake
-DUSE_NGHTTP2=OFF        # libnghttp2
-DCURL_USE_LIBSSH2=OFF   # libssh2
-DUSE_LIBIDN2=OFF        # libidn2 + libunistring + libiconv
-DUSE_LIBPSL=OFF         # libpsl
-DCURL_BROTLI=OFF        # auto-enabled if brotli is installed
-DCURL_ZSTD=OFF          # auto-enabled if zstd is installed
```

With these off, `libcurl.a` references nothing beyond OpenSSL and zlib
(nm-verifiable: no undefined `idn2`/`psl`/`ssh2`/`nghttp` symbols). Protocol
disables (FTP/SMTP/...) are optional — they shrink the binary but add no
dependencies, so this crate does not require them.

The env vars `MINGW_PREFIX` / `MSYS2_ROOT` (Windows sysroot discovery) and
`LIBCLANG_PATH` (libclang for bindgen) influence the build; changing them
re-triggers the build script.

## Testing

`cargo test` runs the bindgen-generated layout assertions (every bound struct
is checked against the C compiler's layout at compile time) plus functional
tests that load the real `libchiaki.a`.
