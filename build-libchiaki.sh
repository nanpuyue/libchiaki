#!/usr/bin/env bash
# Build libchiaki (from chiaki-ng) as static libraries + headers.
#
# Validated on: Windows 11 + MSYS2 mingw64 (run inside the MINGW64 shell).
# Linux (apt/dnf) and macOS (brew) paths follow upstream docs.
#
# Output layout (consumed by the libchiaki binding via LIBCHIAKI_PREFIX):
#   $PREFIX/include/chiaki/*.h (+ remote/, config.h, takion.pb.h)
#   $PREFIX/include/pb*.h              (nanopb runtime headers)
#   $PREFIX/lib/libchiaki.a libcurl.a libgf_complete.a
#               libjerasure.a libprotobuf-nanopb.a
#
# Config (environment variables; proxy needs no flags: https_proxy /
# http_proxy are honored automatically by git, pacman and curl):
#
#   LIBCHIAKI_PREFIX    (required) install prefix
#   CHIAKI_VERSION      git tag/branch/commit, default v1.10.0
#   CHIAKI_REPO         default https://github.com/streetpea/chiaki-ng.git
#   CHIAKI_SRC_DIR      default ./chiaki-ng-<version without the leading v>,
#                       matching the directory name of GitHub release source
#                       archives (a non-git dir is used as-is, submodules
#                       must already be populated inside it)
#   BUILD_TYPE          default Release
#   JOBS                default nproc
#   SKIP_DEPS=1         skip system package installation
#
#   --- Python venv for the nanopb generator (macOS only) ---
#   chiaki-ng generates takion.pb.c/.pb.h at build time by running
#   third-party/nanopb/generator/nanopb_generator.py, which is pure Python and
#   needs google.protobuf. Homebrew Python is PEP 668 "externally managed",
#   so pip refuses to install into it; we create a venv next to build-lib
#   inside the (throwaway) source tree instead. Linux/MSYS2 ship
#   python-protobuf as a system package, so they never get a venv.
#   The venv is never cleaned up: it lives and dies with $CHIAKI_SRC_DIR, and
#   a later run reuses it as-is.
#
#   PIP_MODULES         default "protobuf grpcio-tools" (unpinned; both track
#                       latest). grpcio-tools ships its own protoc and nanopb
#                       prefers it over the one on PATH, so leaving both
#                       unpinned keeps protoc and python-protobuf in step with
#                       each other by construction.
#                       If it breaks on an older nanopb submodule, pin back:
#                         PIP_MODULES='protobuf>=5,<6 grpcio-tools>=5,<6' \
#                         PROTOBUF_FORMULA=protobuf@29 ./build-libchiaki.sh
#                       Drop grpcio-tools to force nanopb onto brew's protoc.
#   PROTOBUF_FORMULA    brew formula providing protoc, default "protobuf"
#                       (currently 35.x, pairs with python-protobuf 6.x).
#                       Set to "protobuf@29" for upstream CI's pinned combo,
#                       which also needs PIP_MODULES pinned to 5.x.
#
# Example (MSYS2 MINGW64 shell):
#   export LIBCHIAKI_PREFIX=/e/build/libchiaki-install
#   ./build-libchiaki.sh
#
# Building the Rust binding afterwards (PowerShell, plain shell):
#   $env:LIBCHIAKI_PREFIX = "E:\build\libchiaki-install"
#   $env:PATH = "E:\msys64\mingw64\bin;$env:PATH"
#   cargo +stable-x86_64-pc-windows-gnu build    # toolchain must be the
#                                                # windows-gnu one (MSVC
#                                                # cannot link the .a)
set -euo pipefail

: "${LIBCHIAKI_PREFIX:?set LIBCHIAKI_PREFIX to the install prefix first}"
CHIAKI_VERSION="${CHIAKI_VERSION:-v1.10.0}"
CHIAKI_REPO="${CHIAKI_REPO:-https://github.com/streetpea/chiaki-ng.git}"
# 目录名与 GitHub release 源码包一致 (chiaki-ng-1.10.0), 解压即用。
CHIAKI_SRC_DIR="${CHIAKI_SRC_DIR:-$PWD/chiaki-ng-${CHIAKI_VERSION#v}}"
BUILD_TYPE="${BUILD_TYPE:-Release}"
JOBS="${JOBS:-$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)}"
SKIP_DEPS="${SKIP_DEPS:-0}"

PIP_MODULES="${PIP_MODULES:-protobuf grpcio-tools}"
PROTOBUF_FORMULA="${PROTOBUF_FORMULA:-protobuf}"

if [ "${1:-}" = "-h" ] || [ "${1:-}" = "--help" ]; then
	sed -n '2,/^set /p' "$0"
	exit 0
fi

# Normalize Windows-style paths when running under MSYS2/Cygwin.
normpath() {
	if command -v cygpath >/dev/null 2>&1; then
		cygpath -u "$1"
	else
		printf '%s' "$1"
	fi
}

PREFIX="$(normpath "$LIBCHIAKI_PREFIX")"
SRC="$(normpath "$CHIAKI_SRC_DIR")"
BUILD_DIR="$SRC/build-lib"
# venv for the nanopb generator; sits next to build-lib inside the throwaway
# source tree, so it is discarded together with $SRC and reused across runs.
PYTHON_VENV_DIR="$SRC/build-nanopb-venv"
OS="$(uname -s)"

log() { echo "[build-libchiaki] $*"; }
trap 'echo "[build-libchiaki] FAILED at line $LINENO (step: ${CURRENT_STEP:-?})" >&2' ERR

# ---------- 1. system dependencies ----------
CURRENT_STEP="system dependencies"
MINGW_PKGS="git mingw-w64-x86_64-gcc mingw-w64-x86_64-cmake mingw-w64-x86_64-ninja \
	mingw-w64-x86_64-pkgconf mingw-w64-x86_64-protobuf \
	mingw-w64-x86_64-python mingw-w64-x86_64-python-protobuf \
	mingw-w64-x86_64-openssl mingw-w64-x86_64-opus mingw-w64-x86_64-json-c \
	mingw-w64-x86_64-libevent mingw-w64-x86_64-miniupnpc"

if [ "$SKIP_DEPS" != "1" ]; then
	case "$OS" in
		MINGW* | MSYS* | CYGWIN*)
			# -Syu: MSYS2 禁止 "只刷库不升级" 的部分升级, 会破坏工具链。
			# shellcheck disable=SC2086
			pacman -Syu --noconfirm --needed $MINGW_PKGS
			;;
		Linux*)
			if command -v apt-get >/dev/null 2>&1; then
				sudo apt-get update
				sudo apt-get install -y git cmake ninja-build pkg-config build-essential \
					libssl-dev libopus-dev libjson-c-dev libevent-dev libminiupnpc-dev \
					protobuf-compiler python3 python3-protobuf
			elif command -v dnf >/dev/null 2>&1; then
				sudo dnf install -y git cmake ninja-build pkgconf gcc \
					openssl-devel opus-devel json-c-devel libevent-devel miniupnpc-devel \
					protobuf-compiler python3 python3-protobuf
			else
				echo "need apt-get or dnf to install dependencies" >&2
				exit 1
			fi
			;;
		Darwin*)
			command -v brew >/dev/null 2>&1 || {
				echo "need Homebrew to install dependencies" >&2
				exit 1
			}
			brew install git cmake ninja pkg-config \
				openssl opus json-c libevent miniupnpc "$PROTOBUF_FORMULA" python
			;;
		*)
			echo "unsupported OS: $OS" >&2
			exit 1
			;;
	esac
fi

# ---------- 2. source + submodules ----------
CURRENT_STEP="source checkout"
# Only the submodules needed for a lib-only static build
# (cpp-steam-tools/munit/oboe/borealis are GUI/CLI/test-only).
SUBMODULES="third-party/nanopb third-party/jerasure third-party/gf-complete third-party/curl"

# 非 git 目录 (GitHub release 源码包解压): 子模块必须已随包提供。
submodules_present() {
	local f
	for f in third-party/curl/CMakeLists.txt \
		third-party/nanopb/generator/nanopb_generator.py \
		third-party/jerasure/include/jerasure.h \
		third-party/gf-complete/include/gf_complete.h; do
		[ -f "$SRC/$f" ] || return 1
	done
}

if [ -d "$SRC/.git" ]; then
	log "using existing git source: $SRC"
	# 换 CHIAKI_VERSION 重跑时必须真的切过去 (此前只在全新 clone 时
	# checkout, 已存在的目录会静默构建旧版本)。本地没有该 ref 时先
	# fetch 再试; fetch 失败 (离线) 但本地已有 ref 则无碍。
	if ! git -C "$SRC" checkout -q "$CHIAKI_VERSION" 2>/dev/null; then
		log "fetching to find $CHIAKI_VERSION"
		git -C "$SRC" fetch --tags origin
		git -C "$SRC" checkout -q "$CHIAKI_VERSION"
	fi
else
	log "cloning $CHIAKI_REPO -> $SRC"
	git clone "$CHIAKI_REPO" "$SRC"
	git -C "$SRC" checkout -q "$CHIAKI_VERSION"
fi

if [ -d "$SRC/.git" ]; then
	# update --init 幂等且已就绪时开销极小, 无条件执行。
	# shellcheck disable=SC2086
	git -C "$SRC" submodule update --init $SUBMODULES
	if git -C "$SRC" submodule status $SUBMODULES | grep -q '^[-+U]'; then
		echo "submodules not ready" >&2
		git -C "$SRC" submodule status >&2
		exit 1
	fi
elif submodules_present; then
	log "using non-git source (release archive?): submodules bundled"
else
	echo "source dir $SRC is not a git checkout and lacks submodule contents" >&2
	echo "(GitHub auto-generated archives do NOT include submodules;" >&2
	echo " use a release archive with vendored submodules, or git clone)" >&2
	exit 1
fi
log "submodules ready"

# ---------- 3. CMake flags: lib-only static build ----------
CMAKE_FLAGS="-G Ninja -DCMAKE_BUILD_TYPE=$BUILD_TYPE \
	-DCHIAKI_ENABLE_GUI=OFF \
	-DCHIAKI_ENABLE_CLI=OFF \
	-DCHIAKI_ENABLE_TESTS=OFF \
	-DCHIAKI_ENABLE_SETSU=OFF \
	-DCHIAKI_ENABLE_STEAMDECK_NATIVE=OFF \
	-DCHIAKI_ENABLE_STEAM_SHORTCUT=OFF \
	-DCHIAKI_ENABLE_SPEEX=OFF \
	-DCHIAKI_ENABLE_FFMPEG_DECODER=OFF"

# curl is used by chiaki purely over HTTP/1.1 + WebSocket (holepunch.c).
# Disable every curl feature that would add an external library dependency;
# protocol-only disables are deliberately left out so that a user-built
# libchiaki stays compatible with minimal configuration:
#  - USE_NGHTTP2=OFF        libnghttp2 (chiaki has no HTTP/2 usage)
#  - CURL_USE_LIBSSH2=OFF   libssh2
#  - USE_LIBIDN2=OFF        libidn2 + libunistring + libiconv (curl's
#                           internal idn stub, idn.c.obj, takes over)
#  - CURL_USE_LIBPSL=OFF    libpsl (the cookie engine is off anyway)
#  - CURL_BROTLI/ZSTD/GSSAPI/LIBSSH/RTMP=OFF  optional pickups that cmake
#                           auto-enables when the libs are installed; they
#                           must never silently enter the dependency closure.
CMAKE_FLAGS="$CMAKE_FLAGS -DUSE_NGHTTP2=OFF -DCURL_USE_LIBSSH2=OFF \
	-DUSE_LIBIDN2=OFF -DCURL_USE_LIBPSL=OFF \
	-DCURL_BROTLI=OFF -DCURL_ZSTD=OFF -DUSE_LIBRTMP=OFF \
	-DCURL_USE_GSSAPI=OFF -DCURL_USE_LIBSSH=OFF"

# ---------- 4. Python venv for the nanopb generator (macOS only) ----------
# Homebrew's Python is PEP 668 externally-managed and refuses bare pip
# installs, so the nanopb generator deps go into a venv. Linux/MSYS2 use
# the system python-protobuf package installed in step 1 and skip this.
if [ "$OS" = "Darwin" ]; then
	PY3="$(command -v python3 || true)"
	[ -n "$PY3" ] || { echo "python3 not found" >&2; exit 1; }

	if [ -x "$PYTHON_VENV_DIR/bin/python3" ]; then
		log "reusing venv: $PYTHON_VENV_DIR"
	else
		log "creating venv: $PYTHON_VENV_DIR"
		"$PY3" -m venv "$PYTHON_VENV_DIR"
	fi

	# Word-split on purpose, but `>=5,<6` must never reach the shell as a
	# redirect, so go through an array instead of a bare $PIP_MODULES.
	read -r -a PIP_MODULE_ARRAY <<<"$PIP_MODULES"
	log "installing into venv: ${PIP_MODULES}"
	# pip install is idempotent and fast when requirements are already met, so
	# just run it unconditionally rather than tracking state ourselves.
	"$PYTHON_VENV_DIR/bin/python3" -m pip install --quiet --disable-pip-version-check \
		"${PIP_MODULE_ARRAY[@]}"

	# Show what nanopb will actually pick up: which protoc, which
	# python-protobuf. This is the one command that catches version mismatches
	# before a full build. Keep an eye on the two version lines.
	"$PYTHON_VENV_DIR/bin/python3" \
		"$SRC/third-party/nanopb/generator/nanopb_generator.py" -vV || true

	CMAKE_FLAGS="$CMAKE_FLAGS -DOPENSSL_ROOT_DIR=$(brew --prefix openssl)"
	# keg-only formulae (protobuf@29) are not linked into /opt/homebrew/bin, so
	# prepend their bin explicitly; for the plain `protobuf` formula this is a
	# harmless no-op that keeps the two cases on one code path.
	PROTOC_BIN_DIR="$(brew --prefix "$PROTOBUF_FORMULA" 2>/dev/null || true)/bin"
	if [ -x "$PROTOC_BIN_DIR/protoc" ]; then
		export PATH="$PROTOC_BIN_DIR:$PATH"
	fi
	log "protoc: $(command -v protoc || echo NOT-FOUND)"
	log "protoc version: $(protoc --version 2>/dev/null || echo 'n/a')"

	# chiaki-ng uses the deprecated FindPythonInterp module, which honors
	# PYTHON_EXECUTABLE; Python3_EXECUTABLE is set for the modern FindPython3
	# in case a future release switches over.
	CMAKE_FLAGS="$CMAKE_FLAGS \
		-DPYTHON_EXECUTABLE=$PYTHON_VENV_DIR/bin/python3 \
		-DPython3_EXECUTABLE=$PYTHON_VENV_DIR/bin/python3"
fi

# ---------- 5. configure + build ----------
CURRENT_STEP="cmake configure + build"
# STATIC SWITCH: MINIUPNP_STATICLIB is the only STATIC macro the stack needs.
# miniupnpc_declspec.h forces __declspec(dllimport) on _WIN32 unless it is
# defined, which pins a runtime libminiupnpc.dll dependency; defining it makes
# holepunch.c use the plain `upnpDiscover` symbol from the static lib. No other
# library in the stack emits __imp_ references (nm-verified), so no other
# per-library macros are needed.
export CFLAGS="${CFLAGS:-} -ffunction-sections -fdata-sections -DMINIUPNP_STATICLIB"
# shellcheck disable=SC2086
cmake -S "$SRC" -B "$BUILD_DIR" $CMAKE_FLAGS
cmake --build "$BUILD_DIR" --target chiaki-lib -j "$JOBS"

# ---------- 6. collect into $PREFIX ----------
CURRENT_STEP="collect into $PREFIX"
log "collecting into $PREFIX"
rm -rf "$PREFIX/include/chiaki"
rm -f "$PREFIX/lib/libchiaki.a" "$PREFIX/lib/libcurl.a" \
	"$PREFIX/lib/libgf_complete.a" "$PREFIX/lib/libjerasure.a" \
	"$PREFIX/lib/libprotobuf-nanopb.a" "$PREFIX/lib/libcpp-steam-tools.dll.a"
mkdir -p "$PREFIX/include/chiaki/remote" "$PREFIX/lib"

cp "$SRC/lib/include/chiaki/"*.h "$PREFIX/include/chiaki/"
# Only built when the matching option is ON (both OFF here).
rm -f "$PREFIX/include/chiaki/ffmpegdecoder.h" "$PREFIX/include/chiaki/pidecoder.h"
cp "$SRC/lib/include/chiaki/remote/"*.h "$PREFIX/include/chiaki/remote/"
cp "$BUILD_DIR/lib/include/chiaki/config.h" "$PREFIX/include/chiaki/config.h"
cp "$BUILD_DIR/lib/protobuf/takion.pb.h" "$PREFIX/include/chiaki/takion.pb.h"
# takion.pb.h does `#include <pb.h>`.
cp "$SRC/third-party/nanopb/pb.h" "$SRC/third-party/nanopb/pb_common.h" \
	"$SRC/third-party/nanopb/pb_decode.h" "$SRC/third-party/nanopb/pb_encode.h" \
	"$PREFIX/include/"
cp "$BUILD_DIR/lib/libchiaki.a" \
	"$BUILD_DIR/third-party/curl/lib/libcurl.a" \
	"$BUILD_DIR/third-party/libgf_complete.a" \
	"$BUILD_DIR/third-party/libjerasure.a" \
	"$BUILD_DIR/third-party/nanopb/libprotobuf-nanopb.a" \
	"$PREFIX/lib/"

# ---------- 7. verify ----------
CURRENT_STEP="verify artifacts"
# 仅做产物存在性与非空检查 (跨平台, 无 binutils 依赖)。符号完整性由
# 绑定库的 tests/link_symbols.rs 在每次 cargo test 时以链接方式保证。
for f in "$PREFIX/lib/libchiaki.a" "$PREFIX/lib/libcurl.a" \
	"$PREFIX/lib/libgf_complete.a" "$PREFIX/lib/libjerasure.a" \
	"$PREFIX/lib/libprotobuf-nanopb.a" \
	"$PREFIX/include/chiaki/session.h" "$PREFIX/include/chiaki/config.h" \
	"$PREFIX/include/chiaki/takion.pb.h" "$PREFIX/include/pb.h"; do
	[ -s "$f" ] || { echo "missing or empty artifact: $f" >&2; exit 1; }
done

log "done."
log "  version : $CHIAKI_VERSION ($BUILD_TYPE)"
log "  prefix  : $PREFIX"
for f in "$PREFIX"/lib/*.a; do
	log "  $(basename "$f"): $(du -h "$f" | cut -f1)"
done
log "  next    : export LIBCHIAKI_PREFIX=$PREFIX && cargo build"
