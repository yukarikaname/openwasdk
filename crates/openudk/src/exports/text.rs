//! Text and language exports: the calls that can answer without a text stack.
//!
//! `Mui_GetFontFallbackLanguageList` can honestly say there are no fallback
//! languages, and `TextAnalysis_SetLocaleNameList` can keep the list a caller sets.
//! The rest need DirectWrite's analysis, bidi data, or the MUI language APIs, so
//! they stay `E_NOTIMPL` rather than invent an answer.

use std::ffi::c_void;
use std::sync::Mutex;

use crate::E_NOTIMPL;

const S_OK: i32 = 0;
const E_FAIL: i32 = 0x8000_4005u32 as i32;
const E_POINTER: i32 = 0x8000_4003u32 as i32;
const E_INVALIDARG: i32 = 0x8007_0057u32 as i32;

/// Read a NUL-terminated UTF-16 string, returning `None` for a null or unterminated
/// pointer.
///
/// # Safety
/// `text` must be null or point to a terminated UTF-16 string.
unsafe fn wide_string(text: *const u16) -> Option<Vec<u16>> {
    if text.is_null() {
        return None;
    }
    let mut len = 0usize;
    while unsafe { *text.add(len) } != 0 {
        len += 1;
        if len > 0x8000 {
            return None;
        }
    }
    Some(unsafe { std::slice::from_raw_parts(text, len) }.to_vec())
}

/// There is no font fallback table in this shim, so the list is empty. Zero is a
/// valid count: a caller learns there are no fallback languages instead of being
/// handed a made-up one.
///
/// # Safety
/// `locale` must point to a NUL-terminated UTF-16 string; `buffer` must be writable
/// for `cch_buffer` UTF-16 units when non-null; `written` must be writable for one
/// `u32`.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn Mui_GetFontFallbackLanguageList(
    locale: *const u16,
    cch_buffer: u32,
    buffer: *mut u16,
    written: *mut u32,
) -> i32 {
    if written.is_null() {
        return E_POINTER;
    }
    if locale.is_null() {
        return E_INVALIDARG;
    }
    if buffer.is_null() && cch_buffer != 0 {
        return E_POINTER;
    }
    // An empty list still terminates: one NUL when there is room for it.
    if !buffer.is_null() && cch_buffer != 0 {
        unsafe { *buffer = 0 };
    }
    unsafe { *written = 0 };
    S_OK
}

struct LocaleLists {
    entries: Vec<(usize, Vec<u16>)>,
}

static LOCALE_LISTS: Mutex<LocaleLists> = Mutex::new(LocaleLists {
    entries: Vec::new(),
});

/// The list belongs to one text analysis source. The source is a caller-owned
/// object, so its address is only a key here; the names themselves are copied.
///
/// # Safety
/// `source` must be the non-null analysis object; `locale_names` must point to a
/// NUL-terminated UTF-16 string.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn TextAnalysis_SetLocaleNameList(
    source: *mut c_void,
    locale_names: *const u16,
) -> i32 {
    if source.is_null() {
        return E_INVALIDARG;
    }
    let Some(names) = (unsafe { wide_string(locale_names) }) else {
        return E_INVALIDARG;
    };
    let Ok(mut lists) = LOCALE_LISTS.lock() else {
        return E_FAIL;
    };
    lists.entries.retain(|(key, _)| *key != source as usize);
    lists.entries.push((source as usize, names));
    S_OK
}

#[cfg(test)]
fn stored_locale_list(source: *mut c_void) -> Option<Vec<u16>> {
    let lists = LOCALE_LISTS.lock().ok()?;
    lists
        .entries
        .iter()
        .find(|(key, _)| *key == source as usize)
        .map(|(_, names)| names.clone())
}

// Creating the proxy needs DirectWrite's private analysis source interface and a
// COM proxy around it. Neither exists in this crate, so no object is returned.
#[unsafe(no_mangle)]
pub extern "system" fn TextAnalysis_CreateDWritePrivateTextAnalysisSourceProxy() -> i32 {
    E_NOTIMPL
}

// Reading direction depends on the text and its locale; without a text stack any
// answer would be a guess, and a wrong direction is worse than a failed call.
#[unsafe(no_mangle)]
pub extern "system" fn TextAnalysis_GetContentReadingDirection() -> i32 {
    E_NOTIMPL
}

// The real export asks the MUI language APIs for the application languages and
// applies them with SetProcessPreferredUILanguages. Keeping a copy here would not
// propagate anything, so the call says it did not.
#[unsafe(no_mangle)]
pub extern "system" fn Mui_PropagateApplicationLanguages() -> i32 {
    E_NOTIMPL
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    #[test]
    fn an_empty_font_fallback_list_is_reported_not_invented() {
        let locale = wide("en-US");
        let mut written = 0xdead_beefu32;
        let mut buffer = [0xffffu16; 4];
        unsafe {
            assert_eq!(
                Mui_GetFontFallbackLanguageList(
                    locale.as_ptr(),
                    buffer.len() as u32,
                    buffer.as_mut_ptr(),
                    &mut written
                ),
                S_OK
            );
        }
        assert_eq!(written, 0);
        assert_eq!(buffer[0], 0);
    }

    #[test]
    fn a_failed_font_fallback_call_leaves_the_count_untouched() {
        let locale = wide("en-US");
        let mut written = 0xdead_beefu32;
        let mut buffer = [0xffffu16; 4];
        unsafe {
            assert_eq!(
                Mui_GetFontFallbackLanguageList(
                    locale.as_ptr(),
                    4,
                    std::ptr::null_mut(),
                    &mut written
                ),
                E_POINTER
            );
            assert_eq!(written, 0xdead_beef);
            assert_eq!(
                Mui_GetFontFallbackLanguageList(
                    std::ptr::null(),
                    4,
                    buffer.as_mut_ptr(),
                    &mut written
                ),
                E_INVALIDARG
            );
            assert_eq!(written, 0xdead_beef);
            assert_eq!(
                Mui_GetFontFallbackLanguageList(
                    locale.as_ptr(),
                    4,
                    buffer.as_mut_ptr(),
                    std::ptr::null_mut()
                ),
                E_POINTER
            );
        }
    }

    #[test]
    fn a_locale_list_round_trips_for_its_source() {
        let source = 0x2000usize as *mut c_void;
        let other = 0x3000usize as *mut c_void;
        let names = wide("en-US;fr-FR");
        let replacement = wide("de-DE");
        unsafe {
            assert_eq!(TextAnalysis_SetLocaleNameList(source, names.as_ptr()), S_OK);
        }
        assert_eq!(
            stored_locale_list(source),
            Some(names[..names.len() - 1].to_vec())
        );
        unsafe {
            assert_eq!(
                TextAnalysis_SetLocaleNameList(other, replacement.as_ptr()),
                S_OK
            );
            // A second call for the same source replaces its list; it does not add one.
            assert_eq!(
                TextAnalysis_SetLocaleNameList(source, replacement.as_ptr()),
                S_OK
            );
        }
        assert_eq!(
            stored_locale_list(source),
            Some(replacement[..replacement.len() - 1].to_vec())
        );
        assert_eq!(
            stored_locale_list(other),
            Some(replacement[..replacement.len() - 1].to_vec())
        );
    }

    #[test]
    fn a_bad_locale_list_call_is_rejected() {
        let source = 0x4000usize as *mut c_void;
        let names = wide("en-US");
        unsafe {
            assert_eq!(
                TextAnalysis_SetLocaleNameList(std::ptr::null_mut(), names.as_ptr()),
                E_INVALIDARG
            );
            assert_eq!(
                TextAnalysis_SetLocaleNameList(source, std::ptr::null()),
                E_INVALIDARG
            );
        }
        assert_eq!(stored_locale_list(source), None);
    }

    #[test]
    fn the_calls_that_need_a_text_stack_say_so() {
        assert_eq!(
            TextAnalysis_CreateDWritePrivateTextAnalysisSourceProxy(),
            E_NOTIMPL
        );
        assert_eq!(TextAnalysis_GetContentReadingDirection(), E_NOTIMPL);
        assert_eq!(Mui_PropagateApplicationLanguages(), E_NOTIMPL);
    }
}
