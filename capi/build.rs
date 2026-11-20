// The shared library names itself, so programs find it by name (and
// rpath) instead of by the path it was linked from.
fn main() {
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    match os.as_str() {
        "linux" | "android" | "freebsd" | "netbsd" | "openbsd" => {
            println!("cargo:rustc-cdylib-link-arg=-Wl,-soname,libdataviz_c.so");
        }
        "macos" | "ios" => {
            println!("cargo:rustc-cdylib-link-arg=-Wl,-install_name,@rpath/libdataviz_c.dylib");
        }
        _ => {}
    }
}
