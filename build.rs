use std::process::Command;

fn main() {
    // Get git information for version
    if let Ok(output) = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
    {
        let git_hash = String::from_utf8(output.stdout).unwrap_or_default();
        println!("cargo:rustc-env=GIT_HASH={}", git_hash.trim());
    } else {
        println!("cargo:rustc-env=GIT_HASH=unknown");
    }

    // Check if repository is dirty
    if let Ok(output) = Command::new("git").args(["diff", "--shortstat"]).output() {
        let is_dirty = !output.stdout.is_empty();
        println!("cargo:rustc-env=GIT_DIRTY={}", is_dirty);
    } else {
        println!("cargo:rustc-env=GIT_DIRTY=false");
    }

    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs");
}
