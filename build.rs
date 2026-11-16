use flate2::{
    Compression,
    write::{DeflateEncoder, GzEncoder},
};
use std::hash::Hasher;
use std::io::Write;
use std::path::{Path, PathBuf};

// libmpv2-sys links `mpv` without a search path. Point the linker at it.
fn main() {
    println!("cargo:rerun-if-env-changed=MPV_LIB_DIR");
    println!("cargo:rerun-if-env-changed=PATH");
    println!("cargo:rerun-if-env-changed=CHYK_SHELL_DIR");
    println!("cargo::rustc-check-cfg=cfg(bundled)");
    println!("cargo::rustc-check-cfg=cfg(shell_ext)");
    let os = std::env::var("CARGO_CFG_TARGET_OS");
    if let Ok(dir) = std::env::var("MPV_LIB_DIR") {
        println!("cargo:rustc-link-search=native={dir}");
    } else if os.as_deref() == Ok("macos") {
        println!("cargo:rustc-link-search=native=/opt/homebrew/lib");
        println!("cargo:rustc-link-search=native=/usr/local/lib");
    }
    if os.as_deref() == Ok("windows") {
        icon();
        // libmpv from MPV_LIB_DIR (needed to link anyway), ffmpeg from PATH.
        let mpv = std::env::var_os("MPV_LIB_DIR").map(|d| Path::new(&d).join("libmpv-2.dll"));
        let ffmpeg = std::env::var_os("PATH").and_then(|p| {
            std::env::split_paths(&p)
                .map(|d| d.join("ffmpeg.exe"))
                .find(|f| f.is_file())
        });
        match (mpv.filter(|f| f.is_file()), ffmpeg) {
            (Some(mpv), Some(ffmpeg)) => {
                // The portable build carries them; the installer downloads them (src/bundle.rs).
                let portable = std::env::var_os("CARGO_FEATURE_PORTABLE").is_some();
                let mut files = vec![
                    ("libmpv-2.dll", mpv, portable),
                    ("ffmpeg.exe", ffmpeg, portable),
                ];
                // The Windows 11 menu entry (`shell/`), built first by the packaging scripts.
                let shell = std::env::var_os("CHYK_SHELL_DIR").map(PathBuf::from);
                let shell = shell.map(|d| [d.join("chyk_shell.dll"), d.join("chyk-shell.exe")]);
                match shell {
                    Some([dll, exe]) if dll.is_file() && exe.is_file() => {
                        files
                            .extend([("chyk_shell.dll", dll, true), ("chyk-shell.exe", exe, true)]);
                        println!("cargo:rustc-cfg=shell_ext");
                    }
                    Some(_) => println!(
                        "cargo:warning=CHYK_SHELL_DIR has no chyk_shell.dll/chyk-shell.exe; no Windows 11 menu entry"
                    ),
                    None => {}
                }
                bundle(&files)
            }
            _ => println!(
                "cargo:warning=libmpv-2.dll (MPV_LIB_DIR) or ffmpeg.exe (PATH) not found; chyk.exe will need them next to it"
            ),
        }
    }
}

/// Explorer/shortcut icon: compile assets/chyk.ico into a resource with rc.exe (MSVC dev env) and link it.
fn icon() {
    println!("cargo:rerun-if-changed=assets/chyk.ico");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let ico = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/chyk.ico");
    let (rc, res) = (out.join("chyk.rc"), out.join("chyk.res"));
    std::fs::write(
        &rc,
        format!(
            "1 ICON \"{}\"\n",
            ico.display().to_string().replace('\\', "\\\\")
        ),
    )
    .unwrap();
    match std::process::Command::new("rc.exe")
        .args(["/nologo", "/fo"])
        .arg(&res)
        .arg(&rc)
        .status()
    {
        Ok(s) if s.success() => println!("cargo:rustc-link-arg-bins={}", res.display()),
        _ => println!("cargo:warning=rc.exe failed or not on PATH; chyk.exe will have no icon"),
    }
}

/// Windows: writes `files.rs` for `src/bundle.rs`, pinning each library by SHA-256 and size.
/// Embedded ones (`true`) are deflated into the exe. The others are downloaded at install, from
/// `<name>.<first 16 hex digits of the hash>.gz`; with `CHYK_DEPS_OUT` set, they are written there
/// gzipped under that name, for CI to publish.
fn bundle(files: &[(&str, PathBuf, bool)]) {
    use sha2::Digest;
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let deps_out = std::env::var_os("CHYK_DEPS_OUT").map(PathBuf::from);
    let mut code = String::from("&[\n");
    let mut id = std::hash::DefaultHasher::new();
    for (name, src, embed) in files {
        println!("cargo:rerun-if-changed={}", src.display());
        let data = std::fs::read(src).unwrap_or_else(|e| panic!("{}: {e}", src.display()));
        let sha = format!("{:x}", sha2::Sha256::digest(&data));
        id.write(sha.as_bytes());
        let from = if *embed {
            let dst = std::fs::File::create(out.join(format!("{name}.deflate"))).unwrap();
            let mut enc = DeflateEncoder::new(dst, Compression::default());
            enc.write_all(&data).unwrap();
            enc.finish().unwrap();
            format!(
                "From::Embedded(include_bytes!(concat!(env!(\"OUT_DIR\"), \"/{name}.deflate\")))"
            )
        } else {
            let asset = format!("{name}.{}.gz", &sha[..16]);
            let mut enc = GzEncoder::new(Vec::new(), Compression::default());
            enc.write_all(&data).unwrap();
            let gz = enc.finish().unwrap();
            if let Some(dir) = &deps_out {
                std::fs::create_dir_all(dir).unwrap();
                std::fs::write(dir.join(&asset), &gz).unwrap();
            }
            format!(
                "From::Download {{ asset: {asset:?}, packed: {} }}",
                gz.len()
            )
        };
        code += &format!(
            "    File {{ name: {name:?}, sha256: {sha:?}, size: {}, from: {from} }},\n",
            data.len()
        );
    }
    code += "]\n";
    std::fs::write(out.join("files.rs"), code).unwrap();
    println!("cargo:rustc-env=CHYK_BUNDLE_ID={:016x}", id.finish());
    println!("cargo:rustc-cfg=bundled");
    // libmpv loads on first use, after `bundle` has put it on disk.
    println!("cargo:rustc-link-arg-bins=/DELAYLOAD:libmpv-2.dll");
    println!("cargo:rustc-link-lib=delayimp");
}
