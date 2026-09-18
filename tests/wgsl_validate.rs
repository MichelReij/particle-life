// Temporary WGSL syntax/validation check, used while reworking the lightning
// shaders. cargo/wasm-pack never parse .wgsl contents (they're just embedded
// strings), so this is the only way to catch a shader mistake before loading
// it in an actual browser.

fn validate(path: &str) {
    let source = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path}: read error: {e}"));
    let module = naga::front::wgsl::parse_str(&source)
        .unwrap_or_else(|e| panic!("{path}: parse error:\n{}", e.emit_to_string(&source)));
    let mut validator = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    );
    validator
        .validate(&module)
        .unwrap_or_else(|e| panic!("{path}: validation error: {e}"));
}

#[test]
fn validate_lightning_shaders() {
    validate("src/shaders/lightning_compute.wgsl");
    validate("src/shaders/lightning_frag_buffer.wgsl");
}
