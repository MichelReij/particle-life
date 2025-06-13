use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn test_function() -> u32 {
    42
}

#[wasm_bindgen]
pub struct TestStruct {
    value: u32,
}

#[wasm_bindgen]
impl TestStruct {
    #[wasm_bindgen(constructor)]
    pub fn new() -> TestStruct {
        TestStruct { value: 123 }
    }
    
    #[wasm_bindgen(getter)]
    pub fn value(&self) -> u32 {
        self.value
    }
}
