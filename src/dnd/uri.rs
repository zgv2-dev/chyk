//! `text/uri-list` (RFC 2483), how file managers hand over dragged files: one `file://` URI per line.

use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::PathBuf;

/// The local paths in a URI list. Comments, blank lines and non-file URIs are skipped.
pub(super) fn paths(list: &str) -> Vec<PathBuf> {
    list.lines()
        .map(str::trim)
        .filter_map(|l| l.strip_prefix("file://"))
        // `file://host/path`: the path starts at the first slash after the (usually empty) host.
        .filter_map(|rest| rest.find('/').map(|i| &rest[i..]))
        .map(|p| PathBuf::from(OsString::from_vec(unescape(p))))
        .collect()
}

/// Percent-decoding to raw bytes: Linux paths need not be UTF-8. A malformed escape stays as is.
fn unescape(s: &str) -> Vec<u8> {
    let b = s.as_bytes();
    let hex = |c: &u8| (*c as char).to_digit(16);
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%'
            && let Some([h, l]) = b.get(i + 1..i + 3)
            && let (Some(h), Some(l)) = (hex(h), hex(l))
        {
            out.push((h * 16 + l) as u8);
            i += 3;
        } else {
            out.push(b[i]);
            i += 1;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_uris_become_paths() {
        assert_eq!(
            paths("file:///home/me/My%20Clips/a%C3%A9.mp4\r\n"),
            [PathBuf::from("/home/me/My Clips/aé.mp4")]
        );
        assert_eq!(
            paths(
                "# from nautilus\nfile://localhost/tmp/x.mkv\nhttps://e.com/v.mp4\n\nfile:///a%2"
            ),
            [PathBuf::from("/tmp/x.mkv"), PathBuf::from("/a%2")]
        );
    }
}
