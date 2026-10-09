fn main() {
    // Tauri embeds its Common-Controls v6 manifest into the app binary only, so the
    // `cargo test` harness loads comctl32 v5 and dies with STATUS_ENTRYPOINT_NOT_FOUND
    // (0xc0000139). Declare the same dependency via linker args for every target instead.
    let msvc = std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    let attrs = if msvc {
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' \
             name='Microsoft.Windows.Common-Controls' version='6.0.0.0' \
             processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
        );
        tauri_build::Attributes::new()
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest())
    } else {
        tauri_build::Attributes::new()
    };
    tauri_build::try_build(attrs).expect("failed to run tauri-build");
}
