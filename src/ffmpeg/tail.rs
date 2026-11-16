use super::*;

/// The last lines of ffmpeg's stderr, each cut short: some errors echo a whole filter expression.
pub(crate) fn error_tail(err: &str) -> String {
    let tail: Vec<String> = err
        .lines()
        .filter(|l| !l.trim().is_empty())
        .rev()
        .take(3)
        .map(|l| {
            let l = l.trim();
            if l.chars().count() > 160 {
                format!("{}…", l.chars().take(160).collect::<String>())
            } else {
                l.to_string()
            }
        })
        .collect();
    if tail.is_empty() {
        t!("error.ffmpeg").into()
    } else {
        tail.into_iter().rev().collect::<Vec<_>>().join("\n")
    }
}
