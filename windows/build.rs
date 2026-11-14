use std::{env, path::PathBuf, process::Command};

fn main() {
    println!("cargo:rerun-if-changed=app.manifest");
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap()).join("app.manifest");
    if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-arg-bin=localdrop=/MANIFEST:EMBED");
        println!(
            "cargo:rustc-link-arg-bin=localdrop=/MANIFESTINPUT:{}",
            manifest.display()
        );
    } else {
        let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
        let rc = out.join("app.rc");
        let object = out.join("app-resource.o");
        std::fs::write(
            &rc,
            format!(
                "1 24 \"{}\"\n",
                manifest.display().to_string().replace('\\', "/")
            ),
        )
        .unwrap();
        let status = Command::new("windres")
            .args(["-i"])
            .arg(rc)
            .args(["-O", "coff", "-o"])
            .arg(&object)
            .status()
            .expect("GNU Windows builds require windres");
        assert!(status.success(), "Failed to embed Windows manifest");
        println!("cargo:rustc-link-arg-bin=localdrop={}", object.display());
    }
}
