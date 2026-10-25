// Nhúng manifest (requireAdministrator) và icon vào EXE thông qua res/app.rc.
fn main() {
    println!("cargo:rerun-if-changed=res/app.rc");
    println!("cargo:rerun-if-changed=res/app.manifest");
    println!("cargo:rerun-if-changed=res/icon.ico");
    #[cfg(windows)]
    embed_resource::compile("res/app.rc", embed_resource::NONE);
}
