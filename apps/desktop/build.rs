//! Embeds the Windows executable icon. No-op on other platforms.

fn main() {
    println!("cargo:rerun-if-changed=ferriscan.rc");
    println!("cargo:rerun-if-changed=assets/icon.ico");
    #[cfg(windows)]
    {
        // The icon is cosmetic: a missing resource compiler must not break the build.
        let _ = embed_resource::compile("ferriscan.rc", embed_resource::NONE);
    }
}
