use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn main() {
    println!("cargo:rerun-if-env-changed=PYGCO_WEB_DIST");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR is set by cargo"));
    let embed_dir = out_dir.join("embedded-web");
    let _ = fs::remove_dir_all(&embed_dir);
    fs::create_dir_all(&embed_dir).expect("create embedded web dir");

    if let Some(dist) = web_dist_dir() {
        println!("cargo:rerun-if-changed={}", dist.display());
        copy_dir(&dist, &embed_dir);
    } else {
        fs::create_dir_all(embed_dir.join("assets")).expect("create fallback embedded assets dir");
        fs::write(
            embed_dir.join("index.html"),
            r#"<!doctype html><html><head><title>pygco</title></head><body><div id="root">pygco API is running. Build web/app/dist before release to embed the React UI.</div><script type="module" src="/assets/embedded-placeholder.js"></script></body></html>"#,
        )
        .expect("write fallback embedded index");
        fs::write(
            embed_dir.join("assets/embedded-placeholder.js"),
            "console.log('pygco embedded placeholder');",
        )
        .expect("write fallback embedded script");
    }

    let mut files = Vec::new();
    collect_files(&embed_dir, &embed_dir, &mut files);
    files.sort();
    let mut generated = String::from("static EMBEDDED_WEB_ASSETS: &[EmbeddedAsset] = &[\n");
    for relative in files {
        let absolute = embed_dir.join(&relative);
        generated.push_str(&format!(
            "    EmbeddedAsset {{ path: {:?}, content_type: {:?}, bytes: include_bytes!(r#\"{}\"#) }},\n",
            slash_path(&relative),
            content_type(&relative),
            absolute.display()
        ));
    }
    generated.push_str("];\n");
    fs::write(out_dir.join("embedded_web_assets.rs"), generated)
        .expect("write embedded web asset manifest");
}

fn web_dist_dir() -> Option<PathBuf> {
    if let Ok(path) = env::var("PYGCO_WEB_DIST") {
        let path = PathBuf::from(path);
        if path.join("index.html").is_file() {
            return Some(path);
        }
    }
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let source_tree_dist = manifest_dir.join("../../web/app/dist");
    if source_tree_dist.join("index.html").is_file() {
        Some(source_tree_dist)
    } else {
        None
    }
}

fn copy_dir(from: &Path, to: &Path) {
    for entry in fs::read_dir(from).expect("read web dist dir") {
        let entry = entry.expect("read web dist entry");
        let source = entry.path();
        let target = to.join(entry.file_name());
        if source.is_dir() {
            fs::create_dir_all(&target).expect("create embedded asset subdir");
            copy_dir(&source, &target);
        } else {
            println!("cargo:rerun-if-changed={}", source.display());
            fs::copy(&source, &target).expect("copy embedded web asset");
        }
    }
}

fn collect_files(root: &Path, dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).expect("read embedded asset dir") {
        let entry = entry.expect("read embedded asset entry");
        let path = entry.path();
        if path.is_dir() {
            collect_files(root, &path, files);
        } else {
            files.push(
                path.strip_prefix(root)
                    .expect("asset under root")
                    .to_path_buf(),
            );
        }
    }
}

fn slash_path(path: &Path) -> String {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|ext| ext.to_str()).unwrap_or("") {
        "css" => "text/css; charset=utf-8",
        "html" => "text/html; charset=utf-8",
        "js" => "text/javascript; charset=utf-8",
        "json" => "application/json",
        "map" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "ico" => "image/x-icon",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}
