//! Prisma-compatible cuid (v1 shape): `c` + 24 lowercase base36 characters.
use rand::Rng;
use std::sync::atomic::{AtomicU32, Ordering};

static COUNTER: AtomicU32 = AtomicU32::new(0);

fn base36(mut n: u128, width: usize) -> String {
    const DIGITS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let mut out = vec![b'0'; width];
    for i in (0..width).rev() {
        out[i] = DIGITS[(n % 36) as usize];
        n /= 36;
    }
    String::from_utf8(out).unwrap()
}

pub fn cuid() -> String {
    let millis = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
    let count = COUNTER.fetch_add(1, Ordering::Relaxed) as u128 % (36u128.pow(4));
    let fingerprint = (std::process::id() as u128) % (36u128.pow(4));
    let random: u128 = rand::rng().random_range(0..36u128.pow(8));
    format!("c{}{}{}{}", base36(millis, 8), base36(count, 4), base36(fingerprint, 4), base36(random, 8))
}

#[cfg(test)]
mod tests {
    #[test]
    fn cuid_shape_and_uniqueness() {
        let a = super::cuid();
        let b = super::cuid();
        assert_eq!(a.len(), 25);
        assert!(a.starts_with('c') && a.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()));
        assert_ne!(a, b);
    }
}
