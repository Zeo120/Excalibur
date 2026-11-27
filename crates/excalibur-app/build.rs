use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn main() {
    println!("cargo::rerun-if-env-changed=CEF_PATH");

    #[cfg(any(target_os = "windows", target_os = "linux"))]
    copy_runtime_if_needed();
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn copy_runtime_if_needed() {
    let Ok(out_dir) = env::var("OUT_DIR").map(PathBuf::from) else {
        return;
    };
    let Some(build_dir) = out_dir
        .ancestors()
        .find(|p| p.file_name().is_some_and(|name| name == "build"))
    else {
        return;
    };
    let Some(profile_dir) = build_dir.parent() else {
        return;
    };

    let marker = if cfg!(target_os = "windows") {
        "libcef.dll"
    } else {
        "libcef.so"
    };

    let source_dir = env::var_os("CEF_PATH")
        .map(PathBuf::from)
        .filter(|dir| dir.join(marker).is_file())
        .or_else(|| {
            build_dir
                .join(marker)
                .is_file()
                .then(|| build_dir.to_path_buf())
        });

    let Some(source_dir) = source_dir else {
        return;
    };

    if let Err(err) = sync_runtime_files(&source_dir, profile_dir) {
        println!("cargo::warning=Failed to sync CEF runtime files: {err}");
    }
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn sync_runtime_files(src: &Path, dst: &Path) -> std::io::Result<()> {
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        if !is_cef_runtime_file(name) {
            continue;
        }
        copy_if_changed(&path, &dst.join(name))?;
    }

    let locales_src = src.join("locales");
    if locales_src.is_dir() {
        let locales_dst = dst.join("locales");
        fs::create_dir_all(&locales_dst)?;
        for entry in fs::read_dir(locales_src)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                if let Some(name) = path.file_name() {
                    copy_if_changed(&path, &locales_dst.join(name))?;
                }
            }
        }
    }

    Ok(())
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn is_cef_runtime_file(name: &str) -> bool {
    if name == "archive.json" {
        return false;
    }
    name.ends_with(".dll")
        || name.ends_with(".so")
        || name.contains(".so.")
        || name.ends_with(".pak")
        || name.ends_with(".dat")
        || name.ends_with(".bin")
        || name.ends_with(".json")
}

#[cfg(any(target_os = "windows", target_os = "linux"))]
fn copy_if_changed(src: &Path, dst: &Path) -> std::io::Result<()> {
    let src_len = fs::metadata(src)?.len();
    if let Ok(dst_meta) = fs::metadata(dst) {
        if dst_meta.len() == src_len {
            return Ok(());
        }
    }
    fs::copy(src, dst)?;
    Ok(())
}
