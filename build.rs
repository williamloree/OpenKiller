// Embeds the icon (res/openkiller.rc) and the manifest (common controls v6, DPI per-monitor v2, asInvoker).
fn main() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let manifest = std::path::Path::new(&dir)
        .join("res")
        .join("openkiller.manifest");
    println!("cargo:rerun-if-changed=res/openkiller.manifest");
    println!("cargo:rerun-if-changed=res/openkiller.ico");
    println!("cargo:rerun-if-changed=res/openkiller.rc");
    embed_resource::compile("res/openkiller.rc", embed_resource::NONE)
        .manifest_optional()
        .unwrap();
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        // LOAD_LIBRARY_SEARCH_SYSTEM32 for imported DLLs: never load them from the exe folder.
        println!("cargo:rustc-link-arg-bins=/DEPENDENTLOADFLAG:0x800");
        println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}",
            manifest.display()
        );
    }
}
