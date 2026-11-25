#[no_mangle]
pub extern "C" fn er_alloc(len: usize) -> *mut u8 {
    Box::into_raw(vec![0u8; len].into_boxed_slice()) as *mut u8
}

/// # Safety
/// `ptr` and `len` must come from `er_alloc` or `er_run`.
#[no_mangle]
pub unsafe extern "C" fn er_free(ptr: *mut u8, len: usize) {
    drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(ptr, len)));
}

/// # Safety
/// `ptr` must point to `len` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn er_run(ptr: *const u8, len: usize) -> u64 {
    let bytes = std::slice::from_raw_parts(ptr, len);
    let out = match std::str::from_utf8(bytes) {
        Ok(s) => entityres::run_json(s),
        Err(_) => r#"{"error":"input is not utf-8"}"#.to_string(),
    };
    let out = out.into_bytes().into_boxed_slice();
    let n = out.len();
    let p = Box::into_raw(out) as *mut u8;
    ((p as u64) << 32) | n as u64
}
