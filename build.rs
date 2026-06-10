// VIBE NOTE: gpt-5.5 (via neukgu-chat) wrote this file.
use std::{env, process::Command};

fn main() {
    // Re-run build script when Git HEAD changes.
    println!("cargo:rerun-if-changed=.git/HEAD");

    let git_hash = Command::new("git")
        .args(["rev-parse", "--short=12", "HEAD"])
        .output()
        .ok()
        .and_then(|out| {
            if out.status.success() {
                Some(String::from_utf8_lossy(&out.stdout).trim().to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=GIT_COMMIT_HASH={git_hash}");

    // This is usually "debug" or "release".
    let profile = env::var("PROFILE").unwrap_or_else(|_| "unknown".to_string());
    println!("cargo:rustc-env=BUILD_PROFILE={profile}");
}
