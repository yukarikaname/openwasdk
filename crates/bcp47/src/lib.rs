//! The one `bcp47mrm.dll` function the App SDK's resource manager imports.
//!
//! `Microsoft.Windows.ApplicationModel.Resources.dll` needs `IsWellFormedTag` at load
//! time, and Wine has no `bcp47mrm.dll` at all, so the engine cannot create a window
//! without this file beside it. Validating a tag is not something a shim can do
//! properly, and answering "yes" for a non-empty tag is the conservative answer: a
//! wrong rejection loses a locale, a wrong acceptance only skips a check the caller
//! could have made itself.

/// `BOOL IsWellFormedTag(PCWSTR tag)`: a non-empty string is taken as well formed.
///
/// # Safety
/// `tag` is a NUL-terminated UTF-16 string owned by the caller, or null.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn IsWellFormedTag(tag: *const u16) -> i32 {
    if tag.is_null() {
        return 0;
    }
    i32::from(unsafe { *tag } != 0)
}

/// A DLL with no global state can always be unloaded.
#[unsafe(no_mangle)]
pub extern "system" fn DllCanUnloadNow() -> i32 {
    1
}

#[cfg(test)]
mod tests {
    #[test]
    fn an_empty_or_null_tag_is_not_well_formed() {
        let empty = [0u16];
        assert_eq!(unsafe { super::IsWellFormedTag(empty.as_ptr()) }, 0);
        assert_eq!(unsafe { super::IsWellFormedTag(std::ptr::null()) }, 0);
    }

    #[test]
    fn a_tag_is_accepted() {
        let tag: Vec<u16> = "en-US\0".encode_utf16().collect();
        assert_eq!(unsafe { super::IsWellFormedTag(tag.as_ptr()) }, 1);
    }
}
