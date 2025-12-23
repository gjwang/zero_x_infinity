use std::process::Command;

fn main() {
    let output = Command::new("git")
        .args(&["rev-parse", "--short", "HEAD"])
        .output()
        .ok();

    let git_hash = match output {
        Some(o) if o.status.success() => {
            let hash = String::from_utf8_lossy(&o.stdout).trim().to_string();
            // Check for dirty state
            let dirty = Command::new("git")
                .args(&["diff", "--quiet"])
                .output()
                .map(|o| !o.status.success())
                .unwrap_or(false);
            if dirty {
                format!("{}-dirty", hash)
            } else {
                hash
            }
        }
        _ => "unknown".to_string(),
    };

    println!("cargo:rustc-env=GIT_HASH={}", git_hash);
    println!("cargo:rerun-if-changed=.git/HEAD");
    println!("cargo:rerun-if-changed=.git/refs/heads");
}
