fn main() {
    // Kompilujemy plik zasobów tylko wtedy, gdy budujemy aplikację na Windowsa
    if std::env::var("CARGO_CFG_WINDOWS").is_ok() {
        let _ = embed_resource::compile("icon.rc", embed_resource::NONE);
    }
}
