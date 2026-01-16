use std::env;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-env-changed=ADD_COMMIT");

    if env::var("ADD_COMMIT").is_ok()
        && let Some(hash) = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|o| o.trim().to_string())
        && hash.len() >= 7
    {
        println!("cargo:rustc-env=GIT_COMMIT_SUFFIX=-{}", &hash[..7]);
        println!("cargo:rerun-if-changed=.git/HEAD");
        println!("cargo:rerun-if-changed=.git/index");
    } else {
        println!("cargo:rustc-env=GIT_COMMIT_SUFFIX=");
    }
}
