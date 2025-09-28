use std::env;
use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let assets_dir = manifest_dir.join("../../assets");
    let generated_manifest = assets_dir.join("generated/manifest.json");
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let bundle_path = out_dir.join("manifest.json");

    println!("cargo:rerun-if-changed={}", generated_manifest.display());
    println!("cargo:rerun-if-changed={}/cards", assets_dir.display());
    println!("cargo:rerun-if-changed={}/themes", assets_dir.display());

    if generated_manifest.exists() {
        if let Err(err) = fs::copy(&generated_manifest, &bundle_path) {
            panic!("failed to copy manifest into build output: {err}");
        }
        println!(
            "cargo:rustc-env=HEARTS_ASSET_MANIFEST={}",
            bundle_path.display()
        );
    }
}
