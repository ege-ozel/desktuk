fn main() {
    // Only embed Windows resources (manifest, icon, version info) for Windows targets.
    // embed-resource picks rc.exe (MSVC) or windres (GNU / mingw cross builds) automatically.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        println!("cargo:rerun-if-changed=res/desktuk.rc");
        println!("cargo:rerun-if-changed=res/desktuk.manifest");
        println!("cargo:rerun-if-changed=res/desktuk.ico");
        embed_resource::compile("res/desktuk.rc", embed_resource::NONE).manifest_required().unwrap();
    }
}
