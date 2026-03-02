// Minimal C-library shim for wasm32-unknown-unknown.
//
// The tree-sitter C code links against standard libc functions (malloc, free,
// fprintf, etc.) which are unresolved on wasm32-unknown-unknown and become
// "env" module imports that the browser cannot satisfy.
//
// This module provides `#[no_mangle] extern "C"` implementations so the
// linker resolves them at compile time instead of importing from "env".

use std::alloc::{self, Layout};

// -------------------------------------------------------------------------
// Memory allocation -- delegate to Rust's global allocator
// -------------------------------------------------------------------------

const ALLOC_ALIGN: usize = 16;

#[no_mangle]
pub unsafe extern "C" fn malloc(size: usize) -> *mut u8 {
    if size == 0 {
        return std::ptr::null_mut();
    }
    // Over-allocate by ALLOC_ALIGN bytes to store the size.
    let total = size + ALLOC_ALIGN;
    let layout = match Layout::from_size_align(total, ALLOC_ALIGN) {
        Ok(l) => l,
        Err(_) => return std::ptr::null_mut(),
    };
    let ptr = alloc::alloc(layout);
    if ptr.is_null() {
        return ptr;
    }
    // Store the total size at the start, return ptr offset past it.
    *(ptr as *mut usize) = total;
    ptr.add(ALLOC_ALIGN)
}

#[no_mangle]
pub unsafe extern "C" fn free(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    let real_ptr = ptr.sub(ALLOC_ALIGN);
    let total = *(real_ptr as *const usize);
    let layout = Layout::from_size_align_unchecked(total, ALLOC_ALIGN);
    alloc::dealloc(real_ptr, layout);
}

#[no_mangle]
pub unsafe extern "C" fn calloc(nmemb: usize, size: usize) -> *mut u8 {
    let total_size = match nmemb.checked_mul(size) {
        Some(s) => s,
        None => return std::ptr::null_mut(),
    };
    let ptr = malloc(total_size);
    if !ptr.is_null() {
        std::ptr::write_bytes(ptr, 0, total_size);
    }
    ptr
}

#[no_mangle]
pub unsafe extern "C" fn realloc(ptr: *mut u8, new_size: usize) -> *mut u8 {
    if ptr.is_null() {
        return malloc(new_size);
    }
    if new_size == 0 {
        free(ptr);
        return std::ptr::null_mut();
    }
    let real_ptr = ptr.sub(ALLOC_ALIGN);
    let old_total = *(real_ptr as *const usize);
    let old_usable = old_total - ALLOC_ALIGN;
    let new_total = new_size + ALLOC_ALIGN;
    let old_layout = Layout::from_size_align_unchecked(old_total, ALLOC_ALIGN);
    let new_ptr = alloc::realloc(real_ptr, old_layout, new_total);
    if new_ptr.is_null() {
        return new_ptr;
    }
    *(new_ptr as *mut usize) = new_total;
    // Zero-fill if growing
    let _ = old_usable; // suppress unused warning
    new_ptr.add(ALLOC_ALIGN)
}

// -------------------------------------------------------------------------
// abort
// -------------------------------------------------------------------------

#[no_mangle]
pub extern "C" fn abort() -> ! {
    std::process::abort();
}

// -------------------------------------------------------------------------
// String operations
// -------------------------------------------------------------------------

#[no_mangle]
pub unsafe extern "C" fn strncmp(s1: *const u8, s2: *const u8, n: usize) -> i32 {
    for i in 0..n {
        let a = *s1.add(i);
        let b = *s2.add(i);
        if a != b {
            return (a as i32) - (b as i32);
        }
        if a == 0 {
            return 0;
        }
    }
    0
}

// -------------------------------------------------------------------------
// I/O stubs -- tree-sitter uses these for debug printing; no-op on wasm
// -------------------------------------------------------------------------

// Note: fprintf and snprintf are variadic in C but we provide fixed-arg
// stubs. The linker matches by symbol name, not signature.
#[no_mangle]
pub unsafe extern "C" fn fprintf(_stream: *mut u8, _fmt: *const u8) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn snprintf(_buf: *mut u8, _size: usize, _fmt: *const u8) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn vsnprintf(
    _buf: *mut u8,
    _size: usize,
    _fmt: *const u8,
    _args: *mut u8,
) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn fputs(_s: *const u8, _stream: *mut u8) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn fputc(_c: i32, _stream: *mut u8) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn fwrite(
    _ptr: *const u8,
    _size: usize,
    count: usize,
    _stream: *mut u8,
) -> usize {
    count
}

#[no_mangle]
pub unsafe extern "C" fn fclose(_stream: *mut u8) -> i32 {
    0
}

#[no_mangle]
pub unsafe extern "C" fn fdopen(_fd: i32, _mode: *const u8) -> *mut u8 {
    std::ptr::null_mut()
}

// -------------------------------------------------------------------------
// Time -- stub (tree-sitter uses this for profiling)
// -------------------------------------------------------------------------

#[repr(C)]
pub struct Timespec {
    tv_sec: i32,
    tv_nsec: i32,
}

#[no_mangle]
pub unsafe extern "C" fn clock_gettime(_clk_id: i32, tp: *mut Timespec) -> i32 {
    if !tp.is_null() {
        (*tp).tv_sec = 0;
        (*tp).tv_nsec = 0;
    }
    0
}
