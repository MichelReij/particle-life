// Simple test to verify console_log! macro works for native builds
use particle_life_wasm::*;

fn main() {
    println!("Testing cross-platform logging...");

    // This should call println!() on native and console.log() on web
    console_log!("🦀 Hello from Rust!");
    console_log!("📊 Testing formatting: number={}, text={}", 42, "test");
    console_log!("✅ Cross-platform logging works!");

    println!("Test completed successfully!");
}
