//! WebAssembly exports for the browser front end.
//!
//! The protocol needs no binding generator:
//!
//! 1. `symap_alloc(len)` reserves `len` bytes; JavaScript writes the deck
//!    there as UTF-8.
//! 2. `symap_run(ptr, len)` runs the deck, frees the input, and returns the
//!    byte length of a JSON result (see `json`).
//! 3. `symap_result_ptr()` gives the address of that result, valid until the
//!    next `symap_run` or `symap_import`.
//!
//! `symap_import(ptr, len)` works the same way, taking a JSON import request
//! (see `json::import`) and returning the deck it makes.

use std::cell::RefCell;

thread_local! {
    static RESULT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

#[unsafe(no_mangle)]
pub extern "C" fn symap_alloc(len: usize) -> *mut u8 {
    let mut buf = Vec::<u8>::with_capacity(len.max(1));
    let ptr = buf.as_mut_ptr();
    std::mem::forget(buf);
    ptr
}

/// # Safety
/// `ptr` must come from `symap_alloc(len)` and not have been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn symap_free(ptr: *mut u8, len: usize) {
    drop(unsafe { Vec::from_raw_parts(ptr, 0, len.max(1)) });
}

/// # Safety
/// `ptr` must come from `symap_alloc(len)` with `len` bytes written.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn symap_run(ptr: *mut u8, len: usize) -> usize {
    let input = unsafe { Vec::from_raw_parts(ptr, len, len.max(1)) };
    let text = String::from_utf8_lossy(&input).into_owned();
    drop(input);
    let json = crate::json::run(&text).into_bytes();
    let n = json.len();
    RESULT.with(|r| *r.borrow_mut() = json);
    n
}

/// # Safety
/// `ptr` must come from `symap_alloc(len)` with `len` bytes written.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn symap_import(ptr: *mut u8, len: usize) -> usize {
    let input = unsafe { Vec::from_raw_parts(ptr, len, len.max(1)) };
    let text = String::from_utf8_lossy(&input).into_owned();
    drop(input);
    let json = crate::json::import(&text).into_bytes();
    let n = json.len();
    RESULT.with(|r| *r.borrow_mut() = json);
    n
}

#[unsafe(no_mangle)]
pub extern "C" fn symap_result_ptr() -> *const u8 {
    RESULT.with(|r| r.borrow().as_ptr())
}
