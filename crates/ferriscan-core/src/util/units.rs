//! Byte formatting primitives.
//!
//! The engine formats sizes for logs and CLI output only; the UI uses `Intl` so
//! that separators and unit names follow the active locale. This module stays
//! locale-neutral on purpose.

/// Whether units are powers of 1024 (`KiB`) or 1000 (`kB`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ByteUnits {
    /// 1024-based, IEC names.
    #[default]
    Binary,
    /// 1000-based, SI names.
    Decimal,
}

impl ByteUnits {
    /// Divisor for the unit index.
    #[must_use]
    pub const fn base(self) -> f64 {
        match self {
            Self::Binary => 1024.0,
            Self::Decimal => 1000.0,
        }
    }

    fn suffix(self, index: usize) -> &'static str {
        const BIN: [&str; 6] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB"];
        const DEC: [&str; 6] = ["B", "kB", "MB", "GB", "TB", "PB"];
        let table = match self {
            Self::Binary => BIN,
            Self::Decimal => DEC,
        };
        table.get(index).copied().unwrap_or("B")
    }
}

/// Formats `bytes` with one decimal place above the byte unit.
///
/// Deterministic and allocation-light: this is used in CLI tables and tests,
/// never in the localized UI.
#[must_use]
pub fn format_bytes(bytes: u64, units: ByteUnits) -> String {
    let base = units.base();
    let mut value = bytes as f64;
    let mut index = 0usize;
    while value >= base && index < 5 {
        value /= base;
        index += 1;
    }
    if index == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", units.suffix(index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_binary_units() {
        assert_eq!(format_bytes(0, ByteUnits::Binary), "0 B");
        assert_eq!(format_bytes(999, ByteUnits::Binary), "999 B");
        assert_eq!(format_bytes(1024, ByteUnits::Binary), "1.0 KiB");
        assert_eq!(format_bytes(1536, ByteUnits::Binary), "1.5 KiB");
        assert_eq!(format_bytes(1 << 30, ByteUnits::Binary), "1.0 GiB");
    }

    #[test]
    fn formats_decimal_units() {
        assert_eq!(format_bytes(1000, ByteUnits::Decimal), "1.0 kB");
        assert_eq!(format_bytes(1_500_000, ByteUnits::Decimal), "1.5 MB");
    }
}
