use std::env;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

// Static chiaki stack, in GNU-ld dependency order
// (dependents first: jerasure needs gf_complete, so it comes first).
const STATIC_LIBS: &[&str] = &[
    "chiaki",
    "curl",
    "jerasure",
    "gf_complete",
    "protobuf-nanopb",
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
    // find_libclang 只是在未设置时自动填充一份合适的默认值)。
    println!("cargo:rerun-if-env-changed=LIBCLANG_PATH");
    // LIBCHIAKI_STATIC_LIBS 只影响 Linux/macOS 分支的链接策略, Windows
    // 不读取也不监听它。
    if os == TargetOs::Linux || os == TargetOs::MacOS {
        println!("cargo:rerun-if-env-changed=LIBCHIAKI_STATIC_LIBS");
    }

    // --- link ---
    let (include_dir, lib_dir) = chiaki_prefix();
    link_stack(&lib_dir);
    link_platform_libs(os);

    // --- bindgen ---
    if os == TargetOs::Windows {
        find_libclang();
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

/// 校验并返回 chiaki 开发前缀 (build-libchiaki.sh 的产物,
/// 布局: include/ + lib/)。
fn chiaki_prefix() -> (PathBuf, PathBuf) {
    let prefix = PathBuf::from(env::var("LIBCHIAKI_PREFIX").expect(
        "LIBCHIAKI_PREFIX must point at the chiaki dev prefix produced \
             by build-libchiaki.sh (layout: include/ + lib/)",
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

/// 平台差异全部收敛在这里: 共通依赖库的链接方式 + 各平台特有库。
fn link_platform_libs(os: TargetOs) {
    // 三平台共用的外部依赖: (pkg-config 名, 库名)。Windows 直接取裸名
    // static=; Linux/macOS 走 pkg-config 探测 (下方循环)。
    // pthread/m 不在清单: rustc/std 自行链接 (glibc 2.34 起并入 libc, musl
    // 无独立的 libpthread/libm, 显式链反而破坏全静态); z 是静态 libcurl.a
    // 的真实依赖, 与其余库一视同仁。
    // opus 由 cargo feature 控制 (默认开启): 纯透传场景 (chiaki-stream)
    // 关闭时把它从依赖闭包移除, 此时 C 库须以 CHIAKI_LIB_ENABLE_OPUS=OFF
    // 构建与之对应。
    let pkg_libs: Vec<(&'static str, &[&str])> = {
        let mut libs: Vec<(&'static str, &[&str])> = vec![
            ("openssl", &["ssl", "crypto"]),
            ("json-c", &["json-c"]),
            ("libevent", &["event"]),
            ("miniupnpc", &["miniupnpc"]),
            ("zlib", &["z"]),
        ];
        if env::var_os("CARGO_FEATURE_OPUS").is_some() {
            libs.push(("opus", &["opus"]));
        }
        libs
    };

    match os {
        TargetOs::Windows => {
            // mingw64 的库 (ssl/opus/event 等的 .a 与 .dll.a) 所在目录,
            // MSYS2 布局固定, 由根直接推出。
            match msys2_root() {
                Some(root) => println!(
                    "cargo:rustc-link-search=native={}",
                    root.join("mingw64/lib").display()
                ),
                None => println!(
                    "cargo:warning=MSYS2 not detected (no mingw64/bin on PATH); \
                     add it to PATH (e.g. C:\\msys64\\mingw64\\bin), \
                     otherwise linking ssl/crypto/opus/json-c/miniupnpc/event will fail"
                ),
            }
            // MSYS2 ships both a static <lib>.a and an import <lib>.dll.a.
            // `static=` pins the former so the final exe carries no extra
            // DLL deps beyond the OS. Same bare-name list as pkg_libs'
            // fallback column: third-party static curl's optional deps
            // (ssh2/psl/idn2/unistring/iconv/brotli/zstd) are disabled in
            // build-libchiaki.sh, so libcurl.a references none of them
            // (nm-verified); only zlib survives.
            for &(_, fallback) in &pkg_libs {
                for lib in fallback {
                    println!("cargo:rustc-link-lib=static={lib}");
                }
            }
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
        TargetOs::Linux | TargetOs::MacOS => {
            // 静态/动态策略, LIBCHIAKI_STATIC_LIBS -> static_list:
            //   未设置 -> None, 优先静态: 在 pkg-config 返回的路径里确认
            //            lib<name>.a 才 static= (macOS 的 static= 不保证
            //            静态, SDK 的 .tbd 桩会静默落到动态库); 没确认到
            //            .a 就发裸 -l, 由链接器默认顺序决定 (先动态后静态)
            //   "all"  -> 全部库名 (无条件全静态, musl 全静态场景)
            //   "none" -> 空名单 (全动态)
            //   名单   -> 逗号分隔库名, 名单内 static= 其余 dylib=;
            //            名单外的库名忽略并警告
            let static_env = env::var("LIBCHIAKI_STATIC_LIBS").ok();
            let static_list: Option<Vec<&str>> = static_env.as_deref().map(|v| match v.trim() {
                "all" => pkg_libs
                    .iter()
                    .flat_map(|(_, l)| l.iter().copied())
                    .collect(),
                "none" => Vec::new(),
                v => {
                    let mut list: Vec<&str> = v
                        .split(',')
                        .map(str::trim)
                        .filter(|s| !s.is_empty())
                        .collect();
                    let unknown: Vec<&str> = list
                        .iter()
                        .copied()
                        .filter(|n| !pkg_libs.iter().any(|(_, l)| l.contains(n)))
                        .collect();
                    if !unknown.is_empty() {
                        println!(
                            "cargo:warning=LIBCHIAKI_STATIC_LIBS: ignoring unknown libs: {}",
                            unknown.join(", ")
                        );
                        list.retain(|n| pkg_libs.iter().any(|(_, l)| l.contains(n)));
                    }
                    list
                }
            });
            for &(pc, libs) in &pkg_libs {
                // pkg-config 只负责定位搜索路径; 探测成功不代表有静态库。
                let dirs = match pkg_config::Config::new().cargo_metadata(false).probe(pc) {
                    Ok(lib) => {
                        for p in &lib.link_paths {
                            println!("cargo:rustc-link-search=native={}", p.display());
                        }
                        Some(lib.link_paths)
                    }
                    Err(e) => {
                        println!("cargo:warning=pkg-config: {e}");
                        None
                    }
                };
                for &name in libs {
                    match &static_list {
                        Some(list) => {
                            if list.contains(&name) {
                                println!("cargo:rustc-link-lib=static={name}");
                            } else {
                                println!("cargo:rustc-link-lib=dylib={name}");
                            }
                        }
                        None => {
                            match dirs
                                .iter()
                                .flatten()
                                .find(|d| d.join(format!("lib{name}.a")).is_file())
                            {
                                // .a 确认存在才发 static= (目录已在上面随探测
                                // 路径一起输出)。
                                Some(_) => println!("cargo:rustc-link-lib=static={name}"),
                                // 未确认: 发裸 -l, 交给链接器默认顺序 (先动态,
                                // 找不到静态再退静态), 不替使用者下结论。
                                None => println!("cargo:rustc-link-lib={name}"),
                            }
                        }
                    }
                }
            }
            if os == TargetOs::MacOS {
                println!("cargo:rustc-link-lib=framework=CoreServices");
                println!("cargo:rustc-link-lib=framework=SystemConfiguration");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Windows (MSYS2) 专属: 根定位与 libclang 自动填充。假设机器上只有一个
// MSYS2 安装且布局固定 (<root>/mingw64/{bin,lib}), 其余路径一律由根推出。
// ---------------------------------------------------------------------------

/// MSYS2 根定位: 从 PATH 上识别 <root>/mingw64/bin (构建要求链接器/
/// 编译器在其中), <root>/usr/bin/msys-2.0.dll 是 MSYS2 的签名文件。
/// 定位有代价 (PATH 扫描 + 签名校验), 结果缓存, 只跑一次。
fn msys2_root() -> Option<&'static Path> {
    static MSYS_ROOT: OnceLock<Option<PathBuf>> = OnceLock::new();
    MSYS_ROOT
        .get_or_init(|| {
            env::split_paths(&env::var_os("PATH")?)
                .find(|bin| {
                    bin.file_name().is_some_and(|n| n == "bin")
                        && bin
                            .parent()
                            .and_then(|p| p.file_name())
                            .is_some_and(|n| n == "mingw64")
                        && bin
                            .parent()
                            .and_then(Path::parent)
                            .is_some_and(|root| root.join("usr/bin/msys-2.0.dll").is_file())
                })
                .and_then(|bin| bin.parent().and_then(Path::parent).map(Path::to_path_buf))
        })
        .as_deref()
}

/// LIBCLANG_PATH 未设置时查找 libclang 并填充 (已设置则让路, 不覆盖
/// 用户的选择), 供 clang-sys 加载。只认 mingw64 的 libclang: 与被解析
/// 的 GCC 头文件环境同源 — MSVC 系 libclang 默认 target/内置头不同,
/// 解析 MinGW 头文件会报错。找不到只告警, 让 bindgen 自己报错。
fn find_libclang() {
    if env::var_os("LIBCLANG_PATH").is_some() {
        return;
    }
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(root) = msys2_root() {
        dirs.push(root.join("mingw64/bin"));
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

// ---------------------------------------------------------------------------
// Windows (MSYS2) 专属区块结束
// ---------------------------------------------------------------------------

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
