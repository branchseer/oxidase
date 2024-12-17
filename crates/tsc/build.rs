use std::{env, fs::read_dir, process::Command};

fn main() {
    for js_src in read_dir("js").unwrap() {
        let js_src = js_src.unwrap();
        if js_src.file_name() == "node_modules" || js_src.file_name() == ".DS_Store" {
            continue;
        }
        println!(
            "cargo::rerun-if-changed=js/{}",
            js_src.file_name().to_str().unwrap()
        );
    }
    let status = Command::new("deno")
        .current_dir("js")
        .args([
            "task",
            "build",
            &format!("{}/dist.js", env::var("OUT_DIR").unwrap()),
        ])
        .status()
        .unwrap();
    assert!(status.success());
}
