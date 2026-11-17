//! Locale-independent number/duration formatting helpers.

use std::time::Duration;

/// `m:ss` below one hour, `h:mm:ss` above.
pub fn duration(d: Duration) -> String {
    let secs = d.as_secs_f64().ceil() as u64;
    let (h, m, s) = (secs / 3600, (secs / 60) % 60, secs % 60);
    if h > 0 { format!("{h}:{m:02}:{s:02}") } else { format!("{m}:{s:02}") }
}

/// Throughput in MiB/s with one decimal, using the given decimal separator.
pub fn megabytes_per_second(bytes_per_sec: f64, decimal_sep: &str) -> String {
    let value = bytes_per_sec / (1024.0 * 1024.0);
    format!("{value:.1}").replace('.', decimal_sep)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations() {
        assert_eq!(duration(Duration::from_secs(0)), "0:00");
        assert_eq!(duration(Duration::from_millis(61_200)), "1:02");
        assert_eq!(duration(Duration::from_secs(3725)), "1:02:05");
    }

    #[test]
    fn rates() {
        assert_eq!(megabytes_per_second(1_572_864.0, ","), "1,5");
        assert_eq!(megabytes_per_second(1_572_864.0, "."), "1.5");
    }
}
