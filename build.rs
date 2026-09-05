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

// Windows/macOS 都链接前缀/MSYS2 内的静态库, 这份清单两者一致。
// Linux 走 pkg-config (见 link_platform_deps), 不用这份清单。
const COMMON_DEPS: &[&str] = &["ssl", "crypto", "opus", "json-c", "miniupnpc", "event"];

// Linux: 系统包管理器的依赖, 库名/版本随发行版变化, 交由 pkg-config
// 定位并自动补齐搜索路径与传递依赖。探测失败时退回裸 -l (裸名见第二列),
// 兼容没有 .pc 文件的环境。
const PKG_DEPS: &[(&str, &[&str])] = &[
    ("openssl", &["ssl", "crypto"]),
    ("opus", &["opus"]),
    ("json-c", &["json-c"]),
    ("libevent", &["event"]),
    ("miniupnpc", &["miniupnpc"]),
    ("zlib", &["z"]),
];

#[derive(Clone, Copy, PartialEq)]
enum TargetOs {
    Windows,
    Linux,
    MacOS,
}

fn main() {
    let target = env::var("TARGET").expect("TARGET not set");
    let os = target_os(&target);

    println!("cargo:rerun-if-env-changed=LIBCHIAKI_PREFIX");
    // clang-sys 在所有平台都读它来定位 libclang (Windows 分支的
    // ensure_libclang 只是在未设置时自动填充一份合适的默认值)。
    println!("cargo:rerun-if-env-changed=LIBCLANG_PATH");
    if os == TargetOs::Windows {
        // 这两个只影响 Windows 的 mingw sysroot 探测, 其余平台读它们
        // 没有意义, 不声明以免无谓的重新构建。
        println!("cargo:rerun-if-env-changed=MINGW_PREFIX");
        println!("cargo:rerun-if-env-changed=MSYS2_ROOT");
    }

    // --- link ---
    let (include_dir, lib_dir) = chiaki_prefix();
    link_stack(&lib_dir);
    link_platform_deps(os);

    // --- bindgen ---
    if os == TargetOs::Windows {
        ensure_libclang();
    }
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let bindings = build_bindings(&include_dir, &out);

    // wrap_static_fns 生成的 C 包装: chiaki 头文件里的 `static inline`
    // 辅助函数在 libchiaki.a 里没有符号, bindgen 也生成不了函数体,
    // 这里把 bindgen 写出的包装 C 文件用真实 C 编译器编译成符号。
    cc::Build::new()
        .file(out.join("__bindgen.c"))
        .include(include_dir)
        .opt_level(2)
        .warnings(false)
        .compile("bindgen_wrappers");

    bindings
        .write_to_file(out.join("bindings.rs"))
        .expect("failed to write bindings");
}

/// 解析 TARGET 三元组, 不支持的目标直接失败。
fn target_os(target: &str) -> TargetOs {
    if target.contains("windows") {
        // 预编译的 .a 是 MinGW/COFF 格式, 无法与 MSVC 工具链链接。
        assert!(
            target.contains("gnu"),
            "libchiaki on Windows requires the GNU toolchain: \
             install the x86_64-pc-windows-gnu target, run from the MSYS2 \
             MINGW64 shell (so the mingw linker is on PATH), and build with \
             `cargo build --target x86_64-pc-windows-gnu`. \
             The prebuilt .a archives are MinGW/COFF and cannot link with MSVC."
        );
        TargetOs::Windows
    } else if target.contains("linux") {
        TargetOs::Linux
    } else if target.contains("apple") {
        TargetOs::MacOS
    } else {
        panic!("libchiaki: unsupported target {target}");
    }
}

/// 校验并返回 chiaki 开发前缀 (scripts/build-chiaki.sh 的产物,
/// 布局: include/ + lib/)。
fn chiaki_prefix() -> (PathBuf, PathBuf) {
    let prefix = PathBuf::from(env::var("LIBCHIAKI_PREFIX").expect(
        "LIBCHIAKI_PREFIX must point at the chiaki dev prefix produced \
             by scripts/build-chiaki.sh (layout: include/ + lib/)",
    ));
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
    (include_dir, lib_dir)
}

/// 静态 chiaki 栈, 各平台一致。
fn link_stack(lib_dir: &std::path::Path) {
    println!("cargo:rustc-link-search=native={}", lib_dir.display());
    for lib in STATIC_LIBS {
        println!("cargo:rustc-link-lib=static={lib}");
    }
}

/// 平台差异全部收敛在这里: 共通依赖的链接方式 + 各平台特有依赖。
fn link_platform_deps(os: TargetOs) {
    match os {
        TargetOs::Windows => {
            // Dynamic deps live in the mingw64 sysroot (import libs).
            match find_mingw_lib() {
                Some(mingw_lib) => {
                    println!("cargo:rustc-link-search=native={}", mingw_lib.display())
                }
                None => println!(
                    "cargo:warning=mingw sysroot not found (set MINGW_PREFIX or MSYS2_ROOT); \
                     linking ssl/crypto/opus/json-c/miniupnpc/event may fail"
                ),
            }
            // MSYS2 ships both a static <lib>.a and an import <lib>.dll.a.
            // `static=` pins the former so the final exe carries no extra
            // DLL deps beyond the OS.
            for lib in COMMON_DEPS {
                println!("cargo:rustc-link-lib=static={lib}");
            }
            // Third-party static curl's remaining deps. The optional ones
            // (ssh2/psl/idn2/unistring/iconv/brotli/zstd) are disabled in
            // scripts/build-chiaki.sh, so libcurl.a references none of them
            // (nm-verified); only zlib survives.
            println!("cargo:rustc-link-lib=static=z");
            // OS libs the chiaki stack itself needs: ws2_32 (Winsock:
            // WSAStartup/WSAIoctl), bcrypt (BCryptGenRandom) and advapi32
            // (CryptAcquireContextW rand fallback + event logging + the
            // Windows cert-store CA lookup, all in libcrypto), iphlpapi
            // (GetAdaptersInfo in holepunch.c) — chiaki's own CMake minus
            // wsock32, which nothing references (import-table verified).
            // userenv/shell32/ole32 have zero references in this closure;
            // Rust std supplies its own.
            for lib in ["ws2_32", "bcrypt", "advapi32", "crypt32", "iphlpapi"] {
                println!("cargo:rustc-link-lib={lib}");
            }
        }
        TargetOs::Linux => {
            for &(pc, fallback) in PKG_DEPS {
                match pkg_config::Config::new().probe(pc) {
                    // pkg_config 自己打印 link-lib/link-search 元数据。
                    Ok(_) => {}
                    Err(e) => {
                        println!("cargo:warning=pkg-config: {e}; falling back to bare -l link");
                        for lib in fallback {
                            println!("cargo:rustc-link-lib=dylib={lib}");
                        }
                    }
                }
            }
            for lib in ["pthread", "m"] {
                println!("cargo:rustc-link-lib=dylib={lib}");
            }
        }
        TargetOs::MacOS => {
            for lib in COMMON_DEPS {
                println!("cargo:rustc-link-lib=static={lib}");
            }
            for lib in ["m", "z"] {
                println!("cargo:rustc-link-lib=dylib={lib}");
            }
            println!("cargo:rustc-link-lib=framework=CoreServices");
            println!("cargo:rustc-link-lib=framework=SystemConfiguration");
        }
    }
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
    for d in ["E:/msys64", "C:/msys64", "C:/tools/msys64", "D:/msys64"] {
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
        // Only emit items defined in chiaki headers. Win32/POSIX 类型
        // (CRITICAL_SECTION、sockaddr_storage 等) 被按值嵌入 chiaki 结构体,
        // bindgen 会把它们作为传递依赖完整生成并附带 layout 断言, 无需
        // allowlist; 但只以指针引用的类型 (sockaddr/sockaddr_in) 不会生成,
        // src 里却要用 (见 discovery.rs), 这里按名字放行。
        .allowlist_file(".*/chiaki/.*\\.h")
        .allowlist_type("sockaddr|sockaddr_in")
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
