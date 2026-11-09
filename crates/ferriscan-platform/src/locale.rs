//! The user's display locale as reported by the operating system.

/// BCP-47 tag of the user's locale (`fr-FR`, `zh-CN`), or `None` when it cannot be read.
///
/// Windows does not export the locale through `LANG`, so it is read from the system here; on
/// other platforms the environment-based detection in `ferriscan-i18n` is enough.
#[must_use]
pub fn system_locale() -> Option<String> {
    #[cfg(windows)]
    {
        use windows_sys::Win32::Globalization::GetUserDefaultLocaleName;

        let mut buf = [0u16; 85]; // LOCALE_NAME_MAX_LENGTH
        // SAFETY: `buf` is a writable buffer of the length passed alongside it.
        let len = unsafe { GetUserDefaultLocaleName(buf.as_mut_ptr(), 85) };
        // The returned count includes the terminating NUL.
        let len = usize::try_from(len).ok()?.checked_sub(1)?;
        String::from_utf16(buf.get(..len)?)
            .ok()
            .filter(|s| !s.is_empty())
    }
    #[cfg(not(windows))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn locale_query_does_not_panic() {
        if let Some(tag) = super::system_locale() {
            assert!(tag.contains(|c: char| c.is_ascii_alphabetic()));
        }
    }
}
