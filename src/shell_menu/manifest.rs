//! The sparse package's `AppxManifest.xml` (see `package.rs`).

use super::package::{CLSID, NAME};

/// `AppxManifest.xml` for the package, version `version` (x.y.z), one verb per extension.
pub(crate) fn manifest(version: &str, exts: &[&str]) -> String {
    let types: String = exts
        .iter()
        .map(|e| {
            format!(
                "            <desktop5:ItemType Type=\".{e}\"><desktop5:Verb Id=\"Chyk\" Clsid=\"{CLSID}\"/></desktop5:ItemType>\n"
            )
        })
        .collect();
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10"
  xmlns:uap="http://schemas.microsoft.com/appx/manifest/uap/windows10"
  xmlns:uap10="http://schemas.microsoft.com/appx/manifest/uap/windows10/10"
  xmlns:desktop4="http://schemas.microsoft.com/appx/manifest/desktop/windows10/4"
  xmlns:desktop5="http://schemas.microsoft.com/appx/manifest/desktop/windows10/5"
  xmlns:com="http://schemas.microsoft.com/appx/manifest/com/windows10"
  xmlns:rescap="http://schemas.microsoft.com/appx/manifest/foundation/windows10/restrictedcapabilities"
  IgnorableNamespaces="uap uap10 desktop4 desktop5 com rescap">
  <Identity Name="{NAME}" Publisher="CN=Chyk" Version="{version}.0" ProcessorArchitecture="x64"/>
  <Properties>
    <DisplayName>Chyk</DisplayName>
    <PublisherDisplayName>Chyk</PublisherDisplayName>
    <Logo>chyk.png</Logo>
    <uap10:AllowExternalContent>true</uap10:AllowExternalContent>
  </Properties>
  <Resources><Resource Language="en-us"/></Resources>
  <Dependencies>
    <TargetDeviceFamily Name="Windows.Desktop" MinVersion="10.0.22000.0" MaxVersionTested="10.0.26100.0"/>
  </Dependencies>
  <Capabilities>
    <rescap:Capability Name="runFullTrust"/>
    <rescap:Capability Name="unvirtualizedResources"/>
  </Capabilities>
  <Applications>
    <Application Id="Chyk" Executable="chyk-shell.exe" uap10:TrustLevel="mediumIL" uap10:RuntimeBehavior="win32App">
      <uap:VisualElements AppListEntry="none" DisplayName="Chyk" Description="Chyk" BackgroundColor="transparent" Square150x150Logo="chyk.png" Square44x44Logo="chyk.png"/>
      <Extensions>
        <desktop4:Extension Category="windows.fileExplorerContextMenus">
          <desktop4:FileExplorerContextMenus>
{types}          </desktop4:FileExplorerContextMenus>
        </desktop4:Extension>
        <com:Extension Category="windows.comServer">
          <com:ComServer>
            <com:SurrogateServer DisplayName="Chyk menu">
              <com:Class Id="{CLSID}" Path="chyk_shell.dll" ThreadingModel="STA"/>
            </com:SurrogateServer>
          </com:ComServer>
        </com:Extension>
      </Extensions>
    </Application>
  </Applications>
</Package>
"#
    )
}

/// Writes the manifest and the icon the package needs into `dir`. A file that already has the
/// right content is left alone, so a folder the user can't write to (Program Files, filled by
/// setup) still works.
#[cfg(windows)]
pub(crate) fn write_files(dir: &std::path::Path) -> std::io::Result<()> {
    let files = [
        (
            "AppxManifest.xml",
            manifest(env!("CARGO_PKG_VERSION"), super::exts()).into_bytes(),
        ),
        ("chyk.png", include_bytes!("../../assets/chyk.png").to_vec()),
    ];
    for (name, data) in files {
        let path = dir.join(name);
        if std::fs::read(&path).is_ok_and(|have| have == data) {
            continue;
        }
        std::fs::write(path, data)?;
    }
    Ok(())
}
