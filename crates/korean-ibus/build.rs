use std::env;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set by cargo"));
    let object = out_dir.join("ibus_shim.o");
    let archive = out_dir.join("libibus_shim.a");

    let status = Command::new("cc")
        .args(pkg_config_args("--cflags"))
        .arg("-fPIC")
        .arg("-c")
        .arg("src/ibus_shim.c")
        .arg("-o")
        .arg(&object)
        .status()
        .expect("failed to run cc for ibus shim");
    assert!(status.success(), "failed to compile ibus shim");

    let status = Command::new("ar")
        .arg("crs")
        .arg(&archive)
        .arg(&object)
        .status()
        .expect("failed to run ar for ibus shim");
    assert!(status.success(), "failed to archive ibus shim");

    println!("cargo:rustc-link-search=native={}", out_dir.display());
    println!("cargo:rustc-link-lib=static=ibus_shim");
    for arg in pkg_config_args("--libs") {
        if let Some(path) = arg.strip_prefix("-L") {
            println!("cargo:rustc-link-search=native={path}");
        } else if let Some(lib) = arg.strip_prefix("-l") {
            println!("cargo:rustc-link-lib={lib}");
        }
    }
    println!("cargo:rerun-if-changed=src/ibus_shim.c");
}

fn pkg_config_args(flag: &str) -> Vec<String> {
    let output = Command::new("pkg-config")
        .arg(flag)
        .arg("ibus-1.0")
        .output()
        .expect("pkg-config ibus-1.0 failed");
    assert!(
        output.status.success(),
        "pkg-config ibus-1.0 {flag} did not succeed"
    );
    String::from_utf8(output.stdout)
        .expect("pkg-config output is utf8")
        .split_whitespace()
        .map(ToOwned::to_owned)
        .collect()
}
