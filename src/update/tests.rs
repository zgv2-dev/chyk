use super::check::{last_response, newer, parse_version, tag_from_location};
use super::paths::{Kind, asset_for, cached_file, clean, kind_from, newest_cached};
use std::path::{Path, PathBuf};

#[test]
fn versions() {
    assert_eq!(parse_version("v1.2.3"), Some([1, 2, 3]));
    assert_eq!(parse_version("1.10"), Some([1, 10, 0]));
    for bad in ["", "v", "1.2.x", "1.2.3.4", "latest", "v1..2"] {
        assert_eq!(parse_version(bad), None, "{bad}");
    }
    assert!(newer("v1.10.0", "1.9.9"));
    assert!(newer("v2.0.0", "1.99.99"));
    assert!(!newer("v1.2.0", "1.2.0"));
    assert!(!newer("v1.1.9", "1.2.0"));
    assert!(!newer("garbage", "1.2.0"));
}

#[test]
fn tag_from_redirect() {
    let h = "HTTP/2 302 \r\ndate: x\r\nLocation: https://github.com/zgv2-dev/chyk/releases/tag/v1.2.1\r\n\r\n";
    assert_eq!(tag_from_location(h).as_deref(), Some("v1.2.1"));
    assert_eq!(tag_from_location("HTTP/2 404 \r\n\r\n"), None);
    let releases = "location: https://github.com/zgv2-dev/chyk/releases\r\n";
    assert_eq!(tag_from_location(releases), None);
}

#[test]
fn final_response_of_redirects() {
    let h = "HTTP/2 302 \r\ncontent-length: 0\r\nlocation: https://x\r\n\r\n\
             HTTP/2 200 \r\ncontent-type: application/octet-stream\r\nContent-Length: 35054275\r\n\r\n";
    assert_eq!(last_response(h), (Some(200), Some(35054275)));
    let missing = "HTTP/2 302 \r\ncontent-length: 5\r\n\r\nHTTP/2 404 \r\n\r\n";
    assert_eq!(last_response(missing), (Some(404), None));
}

#[test]
fn install_kinds() {
    let app = Path::new("/Applications/Chyk.app/Contents/MacOS/chyk");
    assert_eq!(
        kind_from("macos", app, None),
        Some(Kind::MacApp("/Applications/Chyk.app".into()))
    );
    assert_eq!(
        kind_from(
            "macos",
            Path::new("/Users/me/chyk/target/release/chyk"),
            None
        ),
        None
    );
    let exe = Path::new(r"C:\Tools\chyk.exe");
    assert_eq!(
        kind_from("windows", exe, None),
        Some(Kind::WindowsExe(exe.into()))
    );
    let ai = PathBuf::from("/home/me/Chyk.AppImage");
    let mounted = Path::new("/tmp/.mount_Chyk/usr/bin/chyk");
    assert_eq!(
        kind_from("linux", mounted, Some(ai.clone())),
        Some(Kind::AppImage(ai))
    );
    assert_eq!(
        kind_from("linux", Path::new("/usr/bin/chyk"), None),
        Some(Kind::Package)
    );
    assert_eq!(
        kind_from("linux", Path::new("/usr/lib/chyk/chyk"), None),
        Some(Kind::Package)
    );
    assert_eq!(
        kind_from(
            "linux",
            Path::new("/home/me/chyk/target/release/chyk"),
            None
        ),
        None
    );
}

#[test]
fn release_file_per_system() {
    let mac = Kind::MacApp("/Applications/Chyk.app".into());
    assert_eq!(asset_for(&mac, "aarch64"), Some("Chyk-macos-arm64.dmg"));
    assert_eq!(asset_for(&mac, "x86_64"), Some("Chyk-macos-x86_64.dmg"));
    assert_eq!(
        asset_for(&Kind::WindowsExe("c.exe".into()), "x86_64"),
        Some("Chyk-windows-x64.exe")
    );
    let ai = Kind::AppImage("Chyk.AppImage".into());
    assert_eq!(asset_for(&ai, "x86_64"), Some("Chyk-linux-x86_64.AppImage"));
    assert_eq!(asset_for(&ai, "aarch64"), None);
    assert_eq!(asset_for(&Kind::Package, "x86_64"), None);
}

#[test]
fn cache_keeps_only_the_newest_download() {
    let dir = std::env::temp_dir().join(format!("chyk-update-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let asset = "Chyk-macos-arm64.dmg";
    for name in [
        "v1.2.1-Chyk-macos-arm64.dmg",
        "v1.10.0-Chyk-macos-arm64.dmg",
        "v1.3.0-Chyk-macos-arm64.part",
        "junk",
    ] {
        std::fs::write(dir.join(name), b"x").unwrap();
    }
    let (tag, file) = newest_cached(&dir, asset).unwrap();
    assert_eq!(tag, "v1.10.0");
    assert_eq!(file, cached_file(&dir, "v1.10.0", asset));
    clean(&dir, Some(&file));
    let left: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .collect();
    assert_eq!(
        left,
        vec![std::ffi::OsString::from("v1.10.0-Chyk-macos-arm64.dmg")]
    );
    let _ = std::fs::remove_dir_all(&dir);
}
