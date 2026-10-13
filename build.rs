use std::{
    env,
    fs,
    path::{Path, PathBuf},
};

fn collect_files(dir: &Path, base: &Path, out: &mut Vec<(String, PathBuf)>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();

            if path.is_dir() {
                collect_files(&path, base, out);
            } else if path.is_file() {
                let rel = path
                    .strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");

                out.push((rel, path));
            }
        }
    }
}

fn main() {
    // =========================================================
    // EdgeNet Native Bridge - Static
    // =========================================================
    println!("cargo:rustc-link-search=native=native/bridge/build-static");
    println!("cargo:rustc-link-lib=static=edgenet_vpn");

    // =========================================================
    // vcpkg static libraries
    // =========================================================
    println!(
        "cargo:rustc-link-search=native=native/vcpkg/installed/x64-mingw-static/lib"
    );

    // OpenSSL
    println!("cargo:rustc-link-lib=static=ssl");
    println!("cargo:rustc-link-lib=static=crypto");
    // CMake's PRIVATE dependencies of the bridge archive are not propagated
    // to the Rust linker, so list its static dependencies explicitly.
    println!("cargo:rustc-link-lib=static=fmt");
    println!("cargo:rustc-link-lib=static=lz4");
    println!("cargo:rustc-link-lib=static=jsoncpp");

    // MinGW C++ runtime
    println!("cargo:rustc-link-lib=static=stdc++");
    println!("cargo:rustc-link-lib=static=gcc");
    println!("cargo:rustc-link-lib=static=gcc_eh");
    println!("cargo:rustc-link-lib=static=winpthread");

    // Windows system libraries
    println!("cargo:rustc-link-lib=setupapi");
    println!("cargo:rustc-link-lib=cfgmgr32");
    println!("cargo:rustc-link-lib=uuid");
    println!("cargo:rustc-link-lib=iphlpapi");
    println!("cargo:rustc-link-lib=ws2_32");
    println!("cargo:rustc-link-lib=advapi32");
    println!("cargo:rustc-link-lib=crypt32");
    println!("cargo:rustc-link-lib=bcrypt");
    println!("cargo:rustc-link-lib=wininet");
    println!("cargo:rustc-link-lib=rpcrt4");
    println!("cargo:rustc-link-lib=wtsapi32");
    println!("cargo:rustc-link-lib=ncrypt");
    println!("cargo:rustc-link-lib=fwpuclnt");
    println!("cargo:rustc-link-lib=ole32");
    println!("cargo:rustc-link-lib=shell32");

    // =========================================================
    // Embed complete TAP driver directory into EdgeNet.exe
    // =========================================================
    let manifest_dir =
        PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());

    let tap_dir =
        manifest_dir.join("dist/windows/drivers/tap");

    println!(
        "cargo:rerun-if-changed={}",
        tap_dir.display()
    );

    if !tap_dir.exists() {
        panic!(
            "TAP driver directory not found: {}",
            tap_dir.display()
        );
    }

    let mut files = Vec::new();
    collect_files(&tap_dir, &tap_dir, &mut files);
    files.sort_by(|a, b| a.0.cmp(&b.0));

    if files.is_empty() {
        panic!(
            "TAP driver directory is empty: {}",
            tap_dir.display()
        );
    }

    let mut generated = String::new();

    generated.push_str(
        "pub static EMBEDDED_TAP_FILES: &[(&str, &[u8])] = &[\n"
    );

    for (relative_name, absolute_path) in files {
        let absolute_path =
            absolute_path.canonicalize().unwrap();

        generated.push_str(&format!(
            "    ({:?}, include_bytes!(r#\"{}\"#)),\n",
            relative_name,
            absolute_path.display()
        ));
    }

    generated.push_str("];\n");

    let out_dir =
        PathBuf::from(env::var("OUT_DIR").unwrap());

    fs::write(
        out_dir.join("embedded_tap.rs"),
        generated,
    )
    .expect("failed to generate embedded_tap.rs");

    // =========================================================
    // Rebuild triggers
    // =========================================================
    println!(
        "cargo:rerun-if-changed=native/bridge/edgenet_vpn.hpp"
    );

    println!(
        "cargo:rerun-if-changed=native/bridge/edgenet_vpn.cpp"
    );

    println!(
        "cargo:rerun-if-changed=native/bridge/tap_manager.cpp"
    );

    println!(
        "cargo:rerun-if-changed=native/bridge/tap_manager.hpp"
    );
    println!("cargo:rerun-if-changed=native/bridge/build-static/libedgenet_vpn.a");
}
