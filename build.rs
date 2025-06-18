use std::process::Command;

fn main() {
    // Generate build timestamp at compile time
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Generate a readable build ID with date and time
    let output = Command::new("date")
        .args(&["+%Y-%m-%d-%H%M"])
        .output()
        .expect("Failed to execute date command");

    let build_id = String::from_utf8(output.stdout)
        .unwrap()
        .trim()
        .to_string();

    // Set environment variables that can be accessed via env! macro
    println!("cargo:rustc-env=BUILD_TIMESTAMP={}", timestamp);
    println!("cargo:rustc-env=BUILD_ID={}", build_id);

    // Rebuild if any source files change
    println!("cargo:rerun-if-changed=src/");
    println!("cargo:rerun-if-changed=build.rs");
}
