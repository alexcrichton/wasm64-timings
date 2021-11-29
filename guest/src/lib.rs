use std::alloc::{self, Layout};

#[no_mangle]
pub unsafe fn malloc(a: usize) -> *mut u8 {
    let layout = Layout::from_size_align(a, std::mem::align_of::<usize>()).unwrap();
    let ptr = alloc::alloc(layout);
    assert!(!ptr.is_null());
    ptr
}

#[no_mangle]
pub unsafe fn free(ptr: *mut u8, size: usize) {
    let layout = Layout::from_size_align(size, std::mem::align_of::<usize>()).unwrap();
    alloc::dealloc(ptr, layout);
}

pub struct Wasm {
    contents: Vec<u8>,
}

#[no_mangle]
pub unsafe fn wat2wasm(ptr: *const u8, len: usize) -> Box<Wasm> {
    let bytes = std::slice::from_raw_parts(ptr, len);
    let contents = wat::parse_bytes(bytes).unwrap();
    Box::new(Wasm {
        contents: contents.to_vec(),
    })
}

#[no_mangle]
pub unsafe fn wasm_ptr(wasm: &Wasm) -> *const u8 {
    wasm.contents.as_ptr()
}

#[no_mangle]
pub unsafe fn wasm_len(wasm: &Wasm) -> usize {
    wasm.contents.len()
}

#[no_mangle]
pub unsafe fn wasm_free(_wasm: Box<Wasm>) {}

#[no_mangle]
pub unsafe fn validate(ptr: *const u8, len: usize) {
    let bytes = std::slice::from_raw_parts(ptr, len);
    wasmparser::Validator::new().validate_all(bytes).unwrap();
}
