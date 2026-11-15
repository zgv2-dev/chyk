//! Thunar's (Xfce) custom actions file, `uca.xml`, with and without Chyk's action.

use super::linux_files::{Entry, exec_arg};

/// Thunar's id for the action, which [`uca_remove`] looks for.
const UCA_ID: &str = "<unique-id>chyk-edit</unique-id>";

fn xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Thunar (Xfce): `uca.xml` with Chyk's action added, replacing an older one.
pub(crate) fn uca_insert(doc: &str, e: &Entry, exts: &[&str]) -> String {
    let patterns: Vec<_> = exts
        .iter()
        .flat_map(|x| [format!("*.{x}"), format!("*.{}", x.to_uppercase())])
        .collect();
    let action = format!(
        "<action>\n\t<icon>{}</icon>\n\t<name>{}</name>\n\t{UCA_ID}\n\t<command>{} %f</command>\n\
         \t<description></description>\n\t<patterns>{}</patterns>\n\t<video-files/>\n</action>\n",
        xml(&e.icon.to_string_lossy()),
        xml(e.label),
        xml(&exec_arg(e.exe).replace(r"\\", r"\")),
        xml(&patterns.join(";"))
    );
    let doc = uca_remove(doc);
    match doc.rfind("</actions>") {
        Some(i) => format!("{}{action}{}", &doc[..i], &doc[i..]),
        None => {
            format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<actions>\n{action}</actions>\n")
        }
    }
}

/// `uca.xml` without Chyk's action.
pub(crate) fn uca_remove(doc: &str) -> String {
    let Some(id) = doc.find(UCA_ID) else {
        return doc.to_string();
    };
    let start = doc[..id].rfind("<action>").unwrap_or(id);
    let end = doc[id..]
        .find("</action>")
        .map_or(doc.len(), |i| id + i + "</action>".len());
    let end = end + usize::from(doc[end..].starts_with('\n'));
    format!("{}{}", &doc[..start], &doc[end..])
}
