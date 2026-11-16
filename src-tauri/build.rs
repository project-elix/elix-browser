fn main() {
    println!("cargo:rerun-if-env-changed=ELIX_UPDATE_BASE");
    println!("cargo:rerun-if-env-changed=ELIX_UPDATE_PUBLIC_KEY");
    tauri_build::build()
}
