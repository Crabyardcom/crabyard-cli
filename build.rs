fn main() {
    println!("cargo:rustc-env=GIT_VERSION={}", env!("CARGO_PKG_VERSION"));
    println!("cargo:rerun-if-changed=Cargo.toml");
}
