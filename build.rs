use std::{env, io::Result, process::Command};

use poem_grpc_build::Config;

fn main() -> Result<()> {
    let mut includes = vec!["./protos".to_string()];
    if let Ok(protoc_include) = env::var("PROTOC_INCLUDE") {
        includes.push(protoc_include);
    }
    // Convert Vec<String> to Vec<&str>
    let includes_ref: Vec<&str> = includes.iter().map(|s| s.as_str()).collect();
    Config::new()
        .file_descriptor_set_path("rustmailer.bin")
        .compile(&["./protos/rustmailer.proto"], &includes_ref)?;

    let output = Command::new("git")
        .args(&["rev-parse", "--short", "HEAD"])
        .output()
        .expect("Failed to get git commit hash");
    let git_hash = String::from_utf8(output.stdout)
        .expect("Invalid UTF-8")
        .trim()
        .to_string();
    println!("cargo:rustc-env=GIT_HASH={}", git_hash);
    Ok(())
}
