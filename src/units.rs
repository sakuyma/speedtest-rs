//! Speed unit parsing, auto-scaling and formatting.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unit {
    Mbps,
    Bps,
    Kbps,
    Gbps,
    BytesPerSec,
    KBytesPerSec,
    MBytesPerSec,
    GBytesPerSec,
    Kibps,
    Mibps,
    Gibps,
    KiBytesPerSec,
    MiBytesPerSec,
    GiBytesPerSec,
    AutoBinaryBits,
    AutoBinaryBytes,
    AutoDecimalBits,
    AutoDecimalBytes,
}

pub fn parse_unit(text: &str) -> Option<Unit> {
    Some(match text {
        "bps" => Unit::Bps,
        "kbps" => Unit::Kbps,
        "Mbps" => Unit::Mbps,
        "Gbps" => Unit::Gbps,
        "B/s" => Unit::BytesPerSec,
        "kB/s" => Unit::KBytesPerSec,
        "MB/s" => Unit::MBytesPerSec,
        "GB/s" => Unit::GBytesPerSec,
        "kibps" => Unit::Kibps,
        "Mibps" => Unit::Mibps,
        "Gibps" => Unit::Gibps,
        "kiB/s" => Unit::KiBytesPerSec,
        "MiB/s" => Unit::MiBytesPerSec,
        "GiB/s" => Unit::GiBytesPerSec,
        "auto-binary-bits" => Unit::AutoBinaryBits,
        "auto-binary-bytes" => Unit::AutoBinaryBytes,
        "auto-decimal-bits" => Unit::AutoDecimalBits,
        "auto-decimal-bytes" => Unit::AutoDecimalBytes,
        _ => return None,
    })
}

fn pick_prefix(value: f64, base: f64, max_idx: usize) -> usize {
    let mut idx = 0usize;
    let mut threshold = base;
    while idx < max_idx && value >= threshold {
        idx += 1;
        threshold *= base;
    }
    idx
}

pub struct SpeedParts {
    pub value: f64,
    pub unit: &'static str,
}

const DECIMAL_BITS: [&str; 4] = ["bps", "kbps", "Mbps", "Gbps"];
const DECIMAL_BYTES: [&str; 4] = ["B/s", "kB/s", "MB/s", "GB/s"];
const BINARY_BITS: [&str; 4] = ["bps", "kibps", "Mibps", "Gibps"];
const BINARY_BYTES: [&str; 4] = ["B/s", "kiB/s", "MiB/s", "GiB/s"];

pub fn speed_parts(bytes_per_sec: f64, unit: Unit) -> SpeedParts {
    let bits = bytes_per_sec * 8.0;
    let (value, unit) = match unit {
        Unit::Bps => (bits, DECIMAL_BITS[0]),
        Unit::Kbps => (bits / 1000f64.powi(1), DECIMAL_BITS[1]),
        Unit::Mbps => (bits / 1000f64.powi(2), DECIMAL_BITS[2]),
        Unit::Gbps => (bits / 1000f64.powi(3), DECIMAL_BITS[3]),
        Unit::BytesPerSec => (bytes_per_sec, DECIMAL_BYTES[0]),
        Unit::KBytesPerSec => (bytes_per_sec / 1000f64.powi(1), DECIMAL_BYTES[1]),
        Unit::MBytesPerSec => (bytes_per_sec / 1000f64.powi(2), DECIMAL_BYTES[2]),
        Unit::GBytesPerSec => (bytes_per_sec / 1000f64.powi(3), DECIMAL_BYTES[3]),
        Unit::Kibps => (bits / 1024f64.powi(1), BINARY_BITS[1]),
        Unit::Mibps => (bits / 1024f64.powi(2), BINARY_BITS[2]),
        Unit::Gibps => (bits / 1024f64.powi(3), BINARY_BITS[3]),
        Unit::KiBytesPerSec => (bytes_per_sec / 1024f64.powi(1), BINARY_BYTES[1]),
        Unit::MiBytesPerSec => (bytes_per_sec / 1024f64.powi(2), BINARY_BYTES[2]),
        Unit::GiBytesPerSec => (bytes_per_sec / 1024f64.powi(3), BINARY_BYTES[3]),
        Unit::AutoDecimalBits => {
            let i = pick_prefix(bits, 1000.0, 3);
            (bits / 1000f64.powi(i as i32), DECIMAL_BITS[i])
        }
        Unit::AutoDecimalBytes => {
            let i = pick_prefix(bytes_per_sec, 1000.0, 3);
            (bytes_per_sec / 1000f64.powi(i as i32), DECIMAL_BYTES[i])
        }
        Unit::AutoBinaryBits => {
            let i = pick_prefix(bits, 1024.0, 3);
            (bits / 1024f64.powi(i as i32), BINARY_BITS[i])
        }
        Unit::AutoBinaryBytes => {
            let i = pick_prefix(bytes_per_sec, 1024.0, 3);
            (bytes_per_sec / 1024f64.powi(i as i32), BINARY_BYTES[i])
        }
    };
    SpeedParts { value, unit }
}

#[allow(dead_code)]
pub fn format_speed(bytes_per_sec: f64, unit: Unit, precision: usize) -> String {
    let p = speed_parts(bytes_per_sec, unit);
    format!("{:.*} {}", precision, p.value, p.unit)
}

const DATA_UNITS: [&str; 5] = ["B", "kB", "MB", "GB", "TB"];

/// Format a byte count using decimal units with one decimal place.
pub fn format_data(bytes: i64) -> String {
    let v = bytes as f64;
    let idx = pick_prefix(v, 1000.0, 4);
    format!("{:.1} {}", v / 1000f64.powi(idx as i32), DATA_UNITS[idx])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_unit_ok() {
        assert_eq!(parse_unit("Mbps"), Some(Unit::Mbps));
        assert_eq!(parse_unit("auto-decimal-bits"), Some(Unit::AutoDecimalBits));
        assert_eq!(parse_unit("nope"), None);
    }

    #[test]
    fn format_speed_values() {
        assert_eq!(format_speed(69692676.0, Unit::Mbps, 2), "557.54 Mbps");
        assert_eq!(format_speed(0.0, Unit::Mbps, 2), "0.00 Mbps");
    }

    #[test]
    fn speed_parts_value() {
        let p = speed_parts(69692676.0, Unit::Mbps);
        assert!((p.value - 557.541408).abs() < 1e-6);
        assert_eq!(p.unit, "Mbps");
    }

    #[test]
    fn format_data_values() {
        assert_eq!(format_data(1023140336), "1.0 GB");
        assert_eq!(format_data(815966616), "816.0 MB");
        assert_eq!(format_data(500), "500.0 B");
    }
}
