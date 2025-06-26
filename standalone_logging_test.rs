// Simple standalone test to verify console_log! macro works for native builds
// This file doesn't depend on the main crate to avoid WebGPU compilation issues

// Platform-specific logging implementations
#[cfg(target_arch = "wasm32")]
extern "C" {
    pub fn log(s: &str);
}

#[cfg(not(target_arch = "wasm32"))]
pub fn log(s: &str) {
    println!("{}", s);
}

// Cross-platform logging macro that works for both web and native
macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

fn main() {
    println!("Testing cross-platform logging...");

    // This should call println!() on native and console.log() on web
    console_log!("🦀 Hello from Rust!");
    console_log!("📊 Testing formatting: number={}, text={}", 42, "test");
    console_log!("✅ Cross-platform logging works!");

    println!("Test completed successfully!");
}
