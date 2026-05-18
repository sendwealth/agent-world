/// CRC32 implementation for WAL data integrity verification.
/// Uses the standard CRC-32 (ISO 3309) polynomial 0xEDB88320.
pub fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFFFFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 == 1 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
        }
    }
    crc ^ 0xFFFFFFFF
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc32_empty() {
        let result = crc32(&[]);
        // CRC32 of empty data
        assert_eq!(result, 0x00000000);
    }

    #[test]
    fn crc32_hello_world() {
        let result = crc32(b"hello world");
        // Known CRC32 value for "hello world"
        assert_eq!(result, 0x0D4A1185);
    }

    #[test]
    fn crc32_deterministic() {
        let data = b"test data for crc32";
        let first = crc32(data);
        let second = crc32(data);
        assert_eq!(first, second);
    }

    #[test]
    fn crc32_different_inputs() {
        let a = crc32(b"input A");
        let b = crc32(b"input B");
        assert_ne!(a, b);
    }
}
