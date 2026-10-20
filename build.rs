//! Where pdx-ws finds the Steamworks `steam_api` library at run time.
//!
//! `steamworks-sys` links `steam_api` dynamically, but only `client` mode
//! calls it. On Windows it is delay-loaded, so `plan`, `steamcmd` mode and
//! `session export` start without `steam_api64.dll`, and `client` mode
//! reports a missing DLL itself. On Linux and macOS the binary looks next to
//! itself, where the release archive puts the library.

fn main() {
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let env = std::env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    match (os.as_str(), env.as_str()) {
        ("windows", "msvc") => {
            println!("cargo:rustc-link-arg=/DELAYLOAD:steam_api64.dll");
            println!("cargo:rustc-link-lib=dylib=delayimp");
        }
        ("linux", _) => println!("cargo:rustc-link-arg=-Wl,-rpath,$ORIGIN"),
        ("macos", _) => println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path"),
        _ => {}
    }
}
