use std::env;
use std::path::PathBuf;

// Static chiaki stack, in GNU-ld dependency order
// (dependents first: jerasure needs gf_complete, so it comes first).
const STATIC_LIBS: &[&str] = &[
    "chiaki",
    "curl",
    "jerasure",
    "gf_complete",
    "protobuf-nanopb",
];

fn main() {
    println!("cargo:rerun-if-env-changed=LIBCHIAKI_PREFIX");
    println!("cargo:rerun-if-env-changed=MINGW_PREFIX");
    println!("cargo:rerun-if-env-changed=MSYS2_ROOT");
    println!("cargo:rerun-if-env-changed=LIBCLANG_PATH");

    let target = env::var("TARGET").expect("TARGET not set");
    let is_windows = target.contains("windows");
    let is_macos = target.contains("apple");
    let is_linux = target.contains("linux");

    if is_windows && !target.contains("gnu") {
        panic!(
            "libchiaki on Windows requires the GNU toolchain: \
             install the x86_64-pc-windows-gnu target, run from the MSYS2 \
             MINGW64 shell (so the mingw linker is on PATH), and build with \
             `cargo build --target x86_64-pc-windows-gnu`. \
             The prebuilt .a archives are MinGW/COFF and cannot link with MSVC."
        );
    }

    let prefix = PathBuf::from(
        env::var("LIBCHIAKI_PREFIX").expect(
            "LIBCHIAKI_PREFIX must point at the chiaki dev prefix produced \
             by scripts/build-chiaki.sh (layout: include/ + lib/)",
        ),
    );
    let include_dir = prefix.join("include");
    let lib_dir = prefix.join("lib");
    for must_exist in [
        include_dir.join("chiaki").join("common.h"),
        include_dir.join("chiaki").join("session.h"),
        include_dir.join("chiaki").join("config.h"),
        lib_dir.join("libchiaki.a"),
    ] {
        assert!(
            must_exist.is_file(),
            "missing {} - is LIBCHIAKI_PREFIX correct?",
            must_exist.display()
        );
    }

    // --- linking ---
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    for lib in STATIC_LIBS {
        println!("cargo:rustc-link-lib=static={lib}");
    }

    if is_windows {
        // Dynamic deps live in the mingw64 sysroot (import libs).
        if let Some(mingw_lib) = find_mingw_lib() {
            println!("cargo:rustc-link-search=native={}", mingw_lib.display());
        } else {
            println!(
                "cargo:warning=mingw sysroot not found (set MINGW_PREFIX or MSYS2_ROOT); \
                 linking ssl/crypto/opus/json-c/miniupnpc/event may fail"
            );
        }
        // Direct deps (chiaki + third-party static curl's own deps:
        // ssh2/psl/idn2/nghttp2/z are what curl was configured with).
        for lib in [
            "ssl",
            "crypto",
            "opus",
            "json-c",
            "miniupnpc",
            "event",
            "ssh2",
            "psl",
            "idn2",
            "unistring",
            "iconv",
            "z",
        ] {
            // MSYS2 ships both a static <lib>.a and an import <lib>.dll.a.
            // `static=` pins the former so the final exe carries no extra
            // DLL deps beyond the OS.
            println!("cargo:rustc-link-lib=static={lib}");
        }
        // Matches chiaki's own CMake (wsock32 ws2_32 bcrypt iphlpapi) plus
        // what the static Schannel curl / OpenSSL ssh2 / OpenSSL need
        // (crypt32, advapi32, userenv, shell32, ole32). These are OS libs in
        // the mingw CRT import libs, so plain (non-static) link is correct.
        for lib in [
            "ws2_32",
            "wsock32",
            "crypt32",
            "bcrypt",
            "iphlpapi",
            "advapi32",
            "userenv",
            "shell32",
            "ole32",
        ] {
            println!("cargo:rustc-link-lib={lib}");
        }
    } else if is_linux {
        for lib in [
            "ssl", "crypto", "opus", "json-c", "miniupnpc", "event", "pthread", "m", "z",
        ] {
            println!("cargo:rustc-link-lib=dylib={lib}");
        }
    } else if is_macos {
        for lib in ["ssl", "crypto", "opus", "json-c", "miniupnpc", "event"] {
            println!("cargo:rustc-link-lib=static={lib}");
        }
        for lib in ["m", "z"] {
            println!("cargo:rustc-link-lib=dylib={lib}");
        }
        println!("cargo:rustc-link-lib=framework=CoreServices");
        println!("cargo:rustc-link-lib=framework=SystemConfiguration");
    } else {
        panic!("libchiaki: unsupported target {target}");
    }

    // --- bindgen ---
    if is_windows {
        ensure_libclang();
    }
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let bindings = build_bindings(&include_dir, &out);
    bindings
        .write_to_file(out.join("bindings.rs"))
        .expect("failed to write bindings");

    // wrap_static_fns 生成的 C 包装: chiaki 头文件里的 `static inline`
    // 辅助函数在 libchiaki.a 里没有符号, bindgen 也生成不了函数体,
    // 这里把 bindgen 写出的包装 C 文件用真实 C 编译器编译成符号。
    cc::Build::new()
        .file(out.join("__bindgen.c"))
        .include(include_dir)
        .opt_level(2)
        .warnings(false)
        .compile("bindgen_wrappers");
}

/// Locate <mingw64>/lib. Never hardcoded to a single drive:
/// MINGW_PREFIX (msys or windows style) -> MSYS2_ROOT -> well-known roots.
fn find_mingw_lib() -> Option<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Ok(p) = env::var("MINGW_PREFIX") {
        if p.starts_with('/') {
            // msys-style (e.g. /mingw64): need the msys root for a win path.
            if let Some(root) = msys_root() {
                roots.push(root.join(p.trim_start_matches('/')));
            }
        } else {
            roots.push(PathBuf::from(p));
        }
    } else if let Some(root) = msys_root() {
        roots.push(root.join("mingw64"));
    }
    for r in roots {
        let lib = r.join("lib");
        if lib.join("libssl.dll.a").is_file() || lib.join("libcrypto.dll.a").is_file() {
            return Some(lib);
        }
    }
    None
}

fn msys_root() -> Option<PathBuf> {
    let mut cands: Vec<PathBuf> = Vec::new();
    if let Ok(r) = env::var("MSYS2_ROOT") {
        cands.push(PathBuf::from(r));
    }
    for d in [
        "E:/msys64",
        "C:/msys64",
        "C:/tools/msys64",
        "D:/msys64",
    ] {
        cands.push(PathBuf::from(d));
    }
    cands
        .into_iter()
        .find(|r| r.join("mingw64").join("bin").join("gcc.exe").is_file())
}

/// Point clang-sys at a libclang before bindgen runs.
/// Prefer mingw64's (same default target/headers as the GCC-built lib).
fn ensure_libclang() {
    if env::var_os("LIBCLANG_PATH").is_some() {
        return;
    }
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Ok(p) = env::var("MINGW_PREFIX") {
        if !p.starts_with('/') {
            dirs.push(PathBuf::from(p).join("bin"));
        }
    }
    if let Some(root) = msys_root() {
        dirs.push(root.join("mingw64").join("bin"));
    }
    if let Ok(pf) = env::var("ProgramFiles") {
        dirs.push(PathBuf::from(pf).join("LLVM").join("bin"));
    }
    for d in dirs {
        if d.join("libclang.dll").is_file() {
            // SAFETY: single-threaded build script, before any threads spawn.
            unsafe { env::set_var("LIBCLANG_PATH", &d) };
            eprintln!("libchiaki: using libclang from {}", d.display());
            return;
        }
    }
    println!("cargo:warning=LIBCLANG_PATH unset and no libclang found; bindgen may fail");
}

/// Run bindgen over every installed chiaki header.
fn build_bindings(include_dir: &std::path::Path, out: &std::path::Path) -> bindgen::Bindings {
    let chiaki_inc = include_dir.join("chiaki");
    let mut headers: Vec<String> = Vec::new();
    let mut top: Vec<_> = std::fs::read_dir(&chiaki_inc)
        .expect("cannot list chiaki headers")
        .map(|e| e.expect("bad dir entry").path())
        .filter(|p| p.extension().map(|x| x == "h").unwrap_or(false))
        .collect();
    top.sort();
    headers.extend(top.into_iter().map(path_str));
    let mut remote: Vec<_> = std::fs::read_dir(chiaki_inc.join("remote"))
        .expect("cannot list chiaki/remote headers")
        .map(|e| e.expect("bad dir entry").path())
        .filter(|p| p.extension().map(|x| x == "h").unwrap_or(false))
        .collect();
    remote.sort();
    headers.extend(remote.into_iter().map(path_str));

    let mut b = bindgen::Builder::default();
    for h in &headers {
        b = b.header(h.as_str());
    }
    b.clang_arg(format!("-I{}", path_str(include_dir)))
        // Only emit items defined in chiaki headers, plus the Win32 value
        // types embedded in them (they would otherwise turn opaque and
        // silently poison every layout containing them: ChiakiMutex,
        // ChiakiTakion, ChiakiSession, ...).
        .allowlist_file(".*/chiaki/.*\\.h")
        .allowlist_type(
            "CRITICAL_SECTION|_RTL_CRITICAL_SECTION|\
             CONDITION_VARIABLE|_RTL_CONDITION_VARIABLE|\
             sockaddr_storage|sockaddr|SOCKADDR|\
             sockaddr_in|SOCKADDR_IN|in_addr|IN_ADDR|\
             in6_addr|IN6_ADDR|ADDRESS_FAMILY|SOCKET",
        )
        // AF_INET/AF_INET6 are macros in the system socket headers with
        // per-OS values (winsock 23 / Linux 10 / BSD 30); take them from the
        // real headers of each target instead of hand-maintaining the table.
        .allowlist_var("AF_INET.*")
        .default_enum_style(bindgen::EnumVariation::Rust {
            non_exhaustive: false,
        })
        .derive_default(true)
        .generate_comments(false)
        // 布局测试随绑定生成 (bindgen 默认开启), cargo test 时逐类型核对。
        // static inline 辅助函数: 生成 C 包装文件 (见 main 末尾的 cc 编译)。
        .wrap_static_fns(true)
        .wrap_static_fns_path(path_str(out.join("__bindgen.c")))
        .generate()
        .expect("bindgen failed")
}

fn path_str(p: impl AsRef<std::path::Path>) -> String {
    // clang prefers forward slashes.
    p.as_ref()
        .to_str()
        .expect("non-utf8 path")
        .replace('\\', "/")
}
