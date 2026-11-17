fn main() {
    println!("cargo:rerun-if-changed=assets/ycopy.rc");
    println!("cargo:rerun-if-changed=assets/ycopy.ico");
    println!("cargo:rerun-if-changed=assets/ycopy.manifest");
    embed_resource::compile("assets/ycopy.rc", embed_resource::NONE)
        .manifest_required()
        .expect("failed to embed Windows resources");
}
