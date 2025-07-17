use std::{io::Result, process::Command};

use poem_grpc_build::Config;

fn main() -> Result<()> {
    Config::new()
        .file_descriptor_set_path("rustmailer.bin")
        .compile(&["./protos/rustmailer.proto"], &["./protos"])?;

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
