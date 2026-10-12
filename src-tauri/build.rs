fn main() {
    // Иконка зашивается в exe на этапе сборки. Без этой строки Cargo не замечает
    // новую иконку и оставляет в exe старую.
    println!("cargo:rerun-if-changed=icons");
    tauri_build::build()
}
