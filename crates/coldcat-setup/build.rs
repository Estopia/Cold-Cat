use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set by Cargo"));
    let payload_dir = env::var_os("COLD_CAT_PAYLOAD_DIR").map(PathBuf::from);
    let files = ["coldcat.exe", "coldcat-daemon.exe", "coldcat-gui.exe"];

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
