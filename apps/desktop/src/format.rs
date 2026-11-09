//! Byte formatting, same rules as the former `format.ts` (English separators).

use ferriscan_persist::Units;

/// Formats `value` with one decimal above the byte unit, honoring the unit convention.
#[must_use]
pub fn bytes(value: u64, units: Units) -> String {
    let (base, names): (f64, &[&str]) = match units {
        Units::Binary => (1024.0, &["B", "KiB", "MiB", "GiB", "TiB", "PiB"]),
        Units::Decimal => (1000.0, &["B", "kB", "MB", "GB", "TB", "PB"]),
    };
    let mut v = value as f64;
    let mut i = 0;
    while v >= base && i < names.len() - 1 {
        v /= base;
        i += 1;
    }
    if i == 0 {
        format!("{value} B")
    } else {
        format!("{v:.1} {}", names[i])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_both_conventions() {
        assert_eq!(bytes(512, Units::Binary), "512 B");
        assert_eq!(bytes(1536, Units::Binary), "1.5 KiB");
        assert_eq!(bytes(1500, Units::Decimal), "1.5 kB");
    }
}
