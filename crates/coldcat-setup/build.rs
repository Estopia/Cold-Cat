use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    let payload_dir = env::var_os("COLD_CAT_PAYLOAD_DIR").map(PathBuf::from);
    let files = ["coldcat.exe", "coldcat-daemon.exe", "coldcat-gui.exe"];

    embed_windows_manifest(&out_dir);

    let mut generated = String::from(
        "#[allow(clippy::match_single_binding)]\npub fn payload_for(name: &str) -> Option<&'static [u8]> {\n    match name {\n",
    );

    if let Some(payload_dir) = payload_dir {
        println!("cargo:rerun-if-env-changed=COLD_CAT_PAYLOAD_DIR");
        for file in files {
            let path = payload_dir.join(file);
            println!("cargo:rerun-if-changed={}", path.display());
            if path.exists() {
                generated.push_str(&format!(
                    "        {file:?} => Some(include_bytes!(r#\"{}\"#)),\n",
                    path.display()
                ));
            }
        }
    }

    generated.push_str("        _ => None,\n    }\n}\n");
    fs::write(out_dir.join("payload.rs"), generated).expect("write generated payload source");
}

fn embed_windows_manifest(out_dir: &Path) {
    let target = env::var("TARGET").unwrap_or_default();
    if !target.contains("windows-msvc") {
        return;
    }

    let manifest_path = out_dir.join("coldcat-setup.manifest");
    fs::write(&manifest_path, SETUP_MANIFEST).expect("write setup manifest");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-link-arg-bin=coldcat-setup=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg-bin=coldcat-setup=/MANIFESTUAC:NO");
    println!(
        "cargo:rustc-link-arg-bin=coldcat-setup=/MANIFESTINPUT:{}",
        manifest_path.display()
    );
}

const SETUP_MANIFEST: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity
    version="0.1.0.0"
    processorArchitecture="*"
    name="ColdCat.Setup"
    type="win32" />
  <description>Cold Cat Setup</description>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false" />
      </requestedPrivileges>
    </security>
  </trustInfo>
  <compatibility xmlns="urn:schemas-microsoft-com:compatibility.v1">
    <application>
      <supportedOS Id="{35138b9a-5d96-4fbd-8e2d-a2440225f93a}" />
      <supportedOS Id="{4a2f28e3-53b9-4441-ba9c-d69d4a4a6e38}" />
      <supportedOS Id="{1f676c76-80e1-4239-95bb-83d0f6d0da78}" />
      <supportedOS Id="{8e0f7a12-bfb3-4fe8-b9a5-48fd50a15a9a}" />
    </application>
  </compatibility>
</assembly>
"#;
