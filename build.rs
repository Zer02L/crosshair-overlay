use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let mut rpaths = BTreeSet::new();

    let search_paths = [
        "/run/current-system/sw/lib",
        "/run/opengl-driver/lib",
        "/lib",
        "/usr/lib64",
        "/usr/lib",
    ];

    for path in &search_paths {
        if Path::new(path).exists() {
            rpaths.insert(PathBuf::from(path));
        }
    }

    // Автоматическое сканирование всех системных симлинков NixOS (/usr/lib64, /lib, /usr/lib)
    // Это гарантирует, что dlopen("libxkbcommon.so.0"), dlopen("libwayland-client.so.0"),
    // fontconfig, vulkan и EGL гарантированно находятся динамическим загрузчиком.
    for dir in &["/usr/lib64", "/lib", "/usr/lib"] {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Ok(target) = std::fs::canonicalize(&path) {
                    if let Some(parent) = target.parent() {
                        if parent.starts_with("/nix/store") {
                            rpaths.insert(parent.to_path_buf());
                        }
                    }
                }
            }
        }
    }

    for path in rpaths {
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", path.display());
    }
}
