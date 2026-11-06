fn main() {
    // Автоматическое добавление системных путей NixOS / Linux в RPATH
    // Это гарантирует, что dlopen("libwayland-client.so.0") и динамический линковщик
    // всегда безошибочно находят библиотеки Wayland и X11.
    let search_paths = [
        "/run/current-system/sw/lib",
        "/run/opengl-driver/lib",
        "/lib",
        "/usr/lib64",
        "/usr/lib",
    ];

    for path in &search_paths {
        if std::path::Path::new(path).exists() {
            println!("cargo:rustc-link-arg=-Wl,-rpath,{}", path);
        }
    }

    // Если /lib/libwayland-client.so.0 является симлинком на nix store,
    // добавляем точный путь в nix store напрямую в RPATH бинарника:
    if let Ok(target) = std::fs::canonicalize("/lib/libwayland-client.so.0") {
        if let Some(parent) = target.parent() {
            println!("cargo:rustc-link-arg=-Wl,-rpath,{}", parent.display());
        }
    }
}
