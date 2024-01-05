use std::env;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

fn main() {
    let version = match env::var("VERSION"){
        Ok(v) => v,
        Err(_) => "0.0".to_string(),
    };

    let git_version = match env::var("GIT_REF") {
        Ok(v) => v,
        Err(_) => "unknown".to_string(),
    };

    println!("cargo:rustc-env=VERSION={}", version);
    println!("cargo:rustc-env=GIT_REF={}", git_version);

    // Put the linker script somewhere the linker can find it
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    fs::File::create(out_dir.join("memory.x"))
        .unwrap()
        .write_all(include_bytes!("memory.x"))
        .unwrap();
    println!("cargo:rustc-link-search={}", out_dir.display());
    println!("cargo:rerun-if-changed=memory.x");
    println!("cargo:rerun-if-changed=Makefile"); // Version is fed via environment variable here
}
