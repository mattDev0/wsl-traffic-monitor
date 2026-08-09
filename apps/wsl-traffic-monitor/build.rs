//! Embeds the Windows version resource and application icon into the executable.
//!
//! Without this the shipped binary has blank file properties and a generic icon,
//! so a user cannot tell which build they are running and a bug report cannot be
//! tied to a version.

fn main() {
    // build.rs runs on the host, so the target must be read from the environment
    // rather than from cfg!(windows) — this crate is routinely cross-compiled to
    // Windows from Linux.
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "windows" {
        return;
    }

    println!("cargo:rerun-if-changed=assets/app.ico");
    println!("cargo:rerun-if-changed=build.rs");

    let mut res = winresource::WindowsResource::new();
    res.set_icon_with_id("assets/app.ico", "1");
    res.set("ProductName", "WSL Traffic Monitor");
    res.set(
        "FileDescription",
        "Real-time WSL2 network traffic monitor for Windows",
    );
    res.set("CompanyName", "WSL Traffic Monitor contributors");
    res.set("LegalCopyright", "Licensed under GPL-3.0-or-later");
    res.set("OriginalFilename", "wsl-traffic-monitor.exe");
    res.set("InternalName", "wsl-traffic-monitor");

    // Fail loudly. A silent skip would ship a binary with no version identity,
    // which is the defect this build script exists to prevent.
    if let Err(e) = res.compile() {
        panic!(
            "failed to embed Windows resources: {e}\n\
             Cross-compiling to Windows needs a resource compiler on PATH \
             (x86_64-w64-mingw32-windres for the -gnu target, rc.exe from the \
             Windows SDK for -msvc)."
        );
    }
}
