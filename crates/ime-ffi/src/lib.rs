//! C ABI for native adapters. See include/bopomofo.h for ownership rules.
use ime_core::{Engine, Event, KeyEvent, Lexicon};
use sha2::{Digest, Sha256};
use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::ptr;
use std::sync::Arc;

pub const BPM_OK: i32 = 0;
pub const BPM_INVALID: i32 = 1;
pub const BPM_DATA: i32 = 2;
pub const BPM_PANIC: i32 = 3;
const MAX_LEXICON_BYTES: usize = 32 * 1024 * 1024;
const MAX_EVENT_BYTES: usize = 4096;

pub struct BpmEngine {
    engine: Engine,
    poisoned: Cell<bool>,
}

#[repr(C)]
pub struct BpmBuffer {
    pub data: *mut u8,
    pub len: usize,
}
impl Default for BpmBuffer {
    fn default() -> Self {
        Self {
            data: ptr::null_mut(),
            len: 0,
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn bpm_abi_version() -> u32 {
    1
}

unsafe fn input<'a>(data: *const u8, len: usize, max: usize) -> Result<&'a str, i32> {
    if len > max || data.is_null() {
        return Err(BPM_INVALID);
    }
    // SAFETY: the caller guarantees this borrowed region is readable for len bytes.
    let bytes = unsafe { std::slice::from_raw_parts(data, len) };
    std::str::from_utf8(bytes).map_err(|_| BPM_DATA)
}

unsafe fn output(out: *mut BpmBuffer, bytes: Vec<u8>) -> Result<(), i32> {
    if out.is_null() {
        return Err(BPM_INVALID);
    }
    // SAFETY: out points to a live, uniquely writable initialized BpmBuffer.
    let dest = unsafe { &mut *out };
    if !dest.data.is_null() || dest.len != 0 {
        return Err(BPM_INVALID);
    }
    let boxed = bytes.into_boxed_slice();
    dest.len = boxed.len();
    dest.data = Box::into_raw(boxed).cast::<u8>();
    Ok(())
}

fn boundary(action: impl FnOnce() -> Result<(), i32>) -> i32 {
    match catch_unwind(AssertUnwindSafe(action)) {
        Ok(Ok(())) => BPM_OK,
        Ok(Err(code)) => code,
        Err(_) => BPM_PANIC,
    }
}

/// Verify artifact identity against its pinned manifest before opening a context.
/// # Safety
/// Both pointers must remain readable for the specified lengths during this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bpm_lexicon_verify(
    data: *const u8,
    len: usize,
    digest: *const u8,
    digest_len: usize,
) -> i32 {
    boundary(|| {
        let text = unsafe { input(data, len, MAX_LEXICON_BYTES) }?;
        let expected = unsafe { input(digest, digest_len, 64) }?;
        if expected.len() != 64 || !expected.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(BPM_DATA);
        }
        let actual = format!("{:x}", Sha256::digest(text.as_bytes()));
        if !expected.eq_ignore_ascii_case(&actual) {
            return Err(BPM_DATA);
        }
        Ok(())
    })
}

/// Create an independent input context from a normalized UTF-8 lexicon.
/// # Safety
/// Inputs must be valid readable regions; out must point to a writable NULL handle.
/// Pointers must not overlap. The caller owns the returned handle and frees it once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bpm_engine_new(
    data: *const u8,
    len: usize,
    out: *mut *mut BpmEngine,
) -> i32 {
    boundary(|| {
        if out.is_null() {
            return Err(BPM_INVALID);
        }
        // SAFETY: out points to an initialized, writable pointer as required above.
        if !unsafe { *out }.is_null() {
            return Err(BPM_INVALID);
        }
        // SAFETY: input validity is the caller's contract.
        let text = unsafe { input(data, len, MAX_LEXICON_BYTES) }?;
        let lexicon = Lexicon::parse(text).map_err(|_| BPM_DATA)?;
        let handle = Box::new(BpmEngine {
            engine: Engine::new(Arc::new(lexicon)),
            poisoned: Cell::new(false),
        });
        // SAFETY: out was validated; ownership transfers to the caller.
        unsafe {
            *out = Box::into_raw(handle);
        }
        Ok(())
    })
}

/// Process one JSON Event and return one JSON Output, including a transient commit.
/// # Safety
/// Handle must be live and exclusively borrowed for this call; calls are serialized.
/// Input must be readable; out must be a writable {NULL,0}; regions must not overlap.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bpm_engine_handle(
    handle: *mut BpmEngine,
    data: *const u8,
    len: usize,
    out: *mut BpmBuffer,
) -> i32 {
    if handle.is_null() || out.is_null() {
        return BPM_INVALID;
    }
    // SAFETY: live, exclusively accessed handle is required by the caller contract.
    let ctx = unsafe { &mut *handle };
    if ctx.poisoned.get() {
        return BPM_PANIC;
    }
    let result = boundary(|| {
        // Validate the destination BEFORE mutating state or generating a commit.
        // SAFETY: caller supplies a readable initialized output buffer.
        let dest = unsafe { &*out };
        if !dest.data.is_null() || dest.len != 0 {
            return Err(BPM_INVALID);
        }
        // SAFETY: caller's input buffer contract.
        let text = unsafe { input(data, len, MAX_EVENT_BYTES) }?;
        let event: Event = serde_json::from_str(text).map_err(|_| BPM_DATA)?;
        let result = ctx.engine.handle(event);
        let bytes = serde_json::to_vec(&result).map_err(|_| BPM_DATA)?;
        // SAFETY: destination was validated and ownership transfers to the caller.
        unsafe { output(out, bytes) }
    });
    if result == BPM_PANIC {
        ctx.poisoned.set(true);
    }
    result
}

/// Return a display snapshot, which never includes a commit.
/// # Safety
/// Handle must be live; all calls on it must be serialized; out is writable {NULL,0}.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bpm_engine_snapshot(handle: *const BpmEngine, out: *mut BpmBuffer) -> i32 {
    if handle.is_null() {
        return BPM_INVALID;
    }
    // SAFETY: caller guarantees a live handle and serialized access.
    let ctx = unsafe { &*handle };
    if ctx.poisoned.get() {
        return BPM_PANIC;
    }
    let result = boundary(|| {
        let bytes = serde_json::to_vec(&ctx.engine.snapshot()).map_err(|_| BPM_DATA)?;
        // SAFETY: caller supplies a writable zero-initialized output buffer.
        unsafe { output(out, bytes) }
    });
    if result == BPM_PANIC {
        ctx.poisoned.set(true);
    }
    result
}

/// Test a JSON KeyEvent without changing context state.
/// # Safety
/// Handle and input must remain valid; out is writable; regions must not overlap.
/// Serialize this call with all other calls using this handle.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bpm_engine_probe(
    handle: *const BpmEngine,
    data: *const u8,
    len: usize,
    out: *mut bool,
) -> i32 {
    if handle.is_null() || out.is_null() {
        return BPM_INVALID;
    }
    // SAFETY: caller guarantees valid exclusive regions and serialized access.
    let ctx = unsafe { &*handle };
    if ctx.poisoned.get() {
        return BPM_PANIC;
    }
    let result = boundary(|| {
        let text = unsafe { input(data, len, MAX_EVENT_BYTES) }?;
        let key: KeyEvent = serde_json::from_str(text).map_err(|_| BPM_DATA)?;
        // SAFETY: out is a valid writable bool pointer.
        unsafe {
            *out = ctx.engine.probe(&key);
        }
        Ok(())
    });
    if result == BPM_PANIC {
        ctx.poisoned.set(true);
    }
    result
}

/// Free one owned handle; NULL is a no-op.
/// # Safety
/// Non-NULL must be returned by bpm_engine_new, not yet freed, with no active calls.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bpm_engine_free(handle: *mut BpmEngine) {
    if !handle.is_null() {
        let _ = catch_unwind(AssertUnwindSafe(|| {
            // SAFETY: the caller transfers sole ownership back to Rust exactly once.
            unsafe {
                drop(Box::from_raw(handle));
            }
        }));
    }
}

/// Free one owned output buffer; {NULL,0} is a no-op.
/// # Safety
/// Non-NULL must be an unchanged buffer returned by this ABI and not yet freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn bpm_buffer_free(buffer: BpmBuffer) {
    if !buffer.data.is_null() {
        let _ = catch_unwind(AssertUnwindSafe(|| {
            let raw = ptr::slice_from_raw_parts_mut(buffer.data, buffer.len);
            // SAFETY: this exact pointer/length was allocated as a boxed slice above.
            unsafe {
                drop(Box::from_raw(raw));
            }
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const DATA: &str = "# bopomofo-lexicon-v1\nㄅㄚ\t八\t100\n";
    unsafe fn create() -> *mut BpmEngine {
        let mut handle = ptr::null_mut();
        assert_eq!(
            unsafe { bpm_engine_new(DATA.as_ptr(), DATA.len(), &mut handle) },
            BPM_OK
        );
        handle
    }
    unsafe fn snapshot(handle: *mut BpmEngine) -> String {
        let mut buffer = BpmBuffer::default();
        assert_eq!(unsafe { bpm_engine_snapshot(handle, &mut buffer) }, BPM_OK);
        let text =
            std::str::from_utf8(unsafe { std::slice::from_raw_parts(buffer.data, buffer.len) })
                .unwrap()
                .to_owned();
        unsafe {
            bpm_buffer_free(buffer);
        }
        text
    }
    #[test]
    fn repeated_lifetimes_and_probe_are_isolated() {
        for _ in 0..100 {
            unsafe {
                let a = create();
                let b = create();
                let before = snapshot(a);
                let key = br#"{"key":"1"}"#;
                let mut consumed = false;
                assert_eq!(
                    bpm_engine_probe(a, key.as_ptr(), key.len(), &mut consumed),
                    BPM_OK
                );
                assert_eq!(snapshot(a), before);
                assert_eq!(snapshot(b), before);
                bpm_engine_free(a);
                bpm_engine_free(b);
            }
        }
    }
    #[test]
    fn invalid_parameters_and_utf8_do_not_mutate() {
        unsafe {
            let handle = create();
            let before = snapshot(handle);
            let mut out = BpmBuffer::default();
            assert_eq!(
                bpm_engine_handle(handle, ptr::null(), 1, &mut out),
                BPM_INVALID
            );
            assert_eq!(
                bpm_engine_handle(handle, [255].as_ptr(), 1, &mut out),
                BPM_DATA
            );
            assert_eq!(
                bpm_engine_handle(handle, b"{}".as_ptr(), 2, &mut out),
                BPM_DATA
            );
            assert_eq!(snapshot(handle), before);
            assert!(out.data.is_null());
            assert_eq!(bpm_engine_snapshot(ptr::null(), &mut out), BPM_INVALID);
            let mut occupied = handle;
            assert_eq!(
                bpm_engine_new(DATA.as_ptr(), DATA.len(), &mut occupied),
                BPM_INVALID
            );
            bpm_engine_free(handle);
            bpm_engine_free(ptr::null_mut());
            bpm_buffer_free(BpmBuffer::default());
        }
    }
    #[test]
    fn artifact_checksum_is_checked_before_loading() {
        let actual = format!("{:x}", Sha256::digest(DATA.as_bytes()));
        unsafe {
            assert_eq!(
                bpm_lexicon_verify(DATA.as_ptr(), DATA.len(), actual.as_ptr(), actual.len()),
                BPM_OK
            );
            let wrong = "0".repeat(64);
            assert_eq!(
                bpm_lexicon_verify(DATA.as_ptr(), DATA.len(), wrong.as_ptr(), wrong.len()),
                BPM_DATA
            );
            assert_eq!(
                bpm_lexicon_verify(DATA.as_ptr(), DATA.len(), wrong.as_ptr(), 63),
                BPM_DATA
            );
            assert_eq!(
                bpm_lexicon_verify(ptr::null(), 0, wrong.as_ptr(), 64),
                BPM_INVALID
            );
        }
    }

    #[test]
    fn boundaries_catch_panics() {
        assert_eq!(boundary(|| panic!("test-only panic")), BPM_PANIC);
    }
    #[test]
    fn utf8_commit_is_transient_and_destination_errors_preserve_state() {
        unsafe {
            let data = "# bopomofo-lexicon-v1\nㄅㄚ\t𠀀👩‍💻e\u{301}\t100\n";
            let mut handle = ptr::null_mut();
            assert_eq!(
                bpm_engine_new(data.as_ptr(), data.len(), &mut handle),
                BPM_OK
            );
            let call = |key: &str| {
                let event = serde_json::json!({"type":"key", "key":key}).to_string();
                let mut out = BpmBuffer::default();
                assert_eq!(
                    bpm_engine_handle(handle, event.as_ptr(), event.len(), &mut out),
                    BPM_OK
                );
                let value: serde_json::Value =
                    serde_json::from_slice(std::slice::from_raw_parts(out.data, out.len)).unwrap();
                bpm_buffer_free(out);
                value
            };
            call("1");
            call("8");
            let out = call("space");
            assert_eq!(out["snapshot"]["preedit"], "𠀀👩‍💻e\u{301}");
            assert_eq!(out["snapshot"]["cursor"], "𠀀👩‍💻e\u{301}".len());
            let before = snapshot(handle);
            let mut occupied = BpmBuffer {
                data: b"occupied".as_ptr().cast_mut(),
                len: 8,
            };
            let enter = br#"{"type":"key","key":"enter"}"#;
            assert_eq!(
                bpm_engine_handle(handle, enter.as_ptr(), enter.len(), &mut occupied),
                BPM_INVALID
            );
            assert_eq!(snapshot(handle), before);
            let committed = call("enter");
            assert_eq!(committed["commit"], "𠀀👩‍💻e\u{301}");
            assert!(call("enter")["commit"].is_null());
            let display: serde_json::Value = serde_json::from_str(&snapshot(handle)).unwrap();
            assert!(display.get("commit").is_none());
            for invalid in [
                br#"{"type":"reset","extra":"sensitive"}"#.as_slice(),
                br#"{"type":"key","key":"1","extra":1}"#.as_slice(),
            ] {
                let mut out = BpmBuffer::default();
                assert_eq!(
                    bpm_engine_handle(handle, invalid.as_ptr(), invalid.len(), &mut out),
                    BPM_DATA
                );
                assert!(out.data.is_null());
            }
            (*handle).poisoned.set(true);
            let mut out = BpmBuffer::default();
            assert_eq!(bpm_engine_snapshot(handle, &mut out), BPM_PANIC);
            assert!(out.data.is_null());
            bpm_engine_free(handle);
        }
    }

    #[test]
    fn malformed_json_fuzz_is_bounded_and_preserves_state() {
        unsafe {
            let handle = create();
            let before = snapshot(handle);
            for length in 0..256 {
                let data = vec![b'!'; length];
                let mut out = BpmBuffer::default();
                assert_eq!(
                    bpm_engine_handle(handle, data.as_ptr(), data.len(), &mut out),
                    BPM_DATA
                );
                assert!(out.data.is_null());
            }
            assert_eq!(snapshot(handle), before);
            bpm_engine_free(handle);
        }
    }
}
