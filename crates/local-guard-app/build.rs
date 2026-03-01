use std::fs;
use std::path::PathBuf;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let version_path = manifest_dir
        .parent()
        .expect("crates dir")
        .parent()
        .expect("workspace root")
        .join("VERSION");

    println!("cargo:rerun-if-changed={}", version_path.display());

    let raw_version = fs::read_to_string(&version_path).expect("read VERSION file");
    let version = raw_version.trim();
    assert!(
        !version.is_empty(),
        "VERSION file must contain non-empty version"
    );

    println!("cargo:rustc-env=LOCAL_GUARD_VERSION={version}");
}
