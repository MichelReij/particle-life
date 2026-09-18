// WGSL syntax/validation check. cargo/wasm-pack never parse .wgsl contents
// (they're just embedded strings), so this is the only way to catch a shader
// mistake before loading it in an actual browser.

fn validate(path: &std::path::Path) {
    let source = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: read error: {e}", path.display()));
    let module = naga::front::wgsl::parse_str(&source)
        .unwrap_or_else(|e| panic!("{}: parse error:\n{}", path.display(), e.emit_to_string(&source)));
    let mut validator = naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::all(),
    );
    validator
        .validate(&module)
        .unwrap_or_else(|e| panic!("{}: validation error: {e}", path.display()));
}

#[test]
fn validate_all_shaders() {
    let dir = std::path::Path::new("src/shaders");
    let mut checked = 0;
    for entry in std::fs::read_dir(dir).expect("read src/shaders") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|e| e.to_str()) == Some("wgsl") {
            validate(&path);
            checked += 1;
        }
    }
    assert!(checked > 0, "no .wgsl files found under {}", dir.display());
}
