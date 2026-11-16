//! Minimal ZIP writer (stored, no compression). Enough to bundle the already
//! compressed installer with a few small text files, without a dependency.

fn crc32(data: &[u8]) -> u32 {
    let mut table = [0u32; 256];
    for (i, slot) in table.iter_mut().enumerate() {
        let mut c = i as u32;
        for _ in 0..8 {
            c = if c & 1 != 0 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
        }
        *slot = c;
    }
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in data {
        crc = table[((crc ^ byte as u32) & 0xFF) as usize] ^ (crc >> 8);
    }
    !crc
}

fn u16le(out: &mut Vec<u8>, v: u16) {
    out.extend_from_slice(&v.to_le_bytes());
}
fn u32le(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

/// Builds an archive from (name, content) pairs. Names must be plain ASCII file names.
pub fn archive(files: &[(&str, &[u8])]) -> Vec<u8> {
    const DATE: u16 = ((2026 - 1980) << 9) | (1 << 5) | 1; // 2026-01-01, fixed for reproducible output
    let mut out = Vec::new();
    let mut directory = Vec::new();
    for (name, data) in files {
        let offset = out.len() as u32;
        let crc = crc32(data);
        let size = data.len() as u32;
        // Local header and central directory entry share these fields.
        let mut common = Vec::new();
        u16le(&mut common, 20); // version needed
        u16le(&mut common, 0); // flags
        u16le(&mut common, 0); // method: stored
        u16le(&mut common, 0); // time
        u16le(&mut common, DATE);
        u32le(&mut common, crc);
        u32le(&mut common, size);
        u32le(&mut common, size);
        u16le(&mut common, name.len() as u16);
        u16le(&mut common, 0); // extra length

        u32le(&mut out, 0x0403_4b50);
        out.extend_from_slice(&common);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);

        u32le(&mut directory, 0x0201_4b50);
        u16le(&mut directory, 20); // version made by
        directory.extend_from_slice(&common);
        u16le(&mut directory, 0); // comment length
        u16le(&mut directory, 0); // disk number
        u16le(&mut directory, 0); // internal attributes
        u32le(&mut directory, 0); // external attributes
        u32le(&mut directory, offset);
        directory.extend_from_slice(name.as_bytes());
    }
    let directory_offset = out.len() as u32;
    out.extend_from_slice(&directory);
    u32le(&mut out, 0x0605_4b50);
    u16le(&mut out, 0);
    u16le(&mut out, 0);
    u16le(&mut out, files.len() as u16);
    u16le(&mut out, files.len() as u16);
    u32le(&mut out, directory.len() as u32);
    u32le(&mut out, directory_offset);
    u16le(&mut out, 0);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn crc_reference_value() {
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }
    #[test]
    fn archive_layout() {
        let zip = archive(&[("a.txt", b"hello"), ("b.cmd", b"")]);
        assert_eq!(&zip[..4], &[0x50, 0x4b, 0x03, 0x04]);
        assert_eq!(&zip[zip.len() - 22..zip.len() - 18], &[0x50, 0x4b, 0x05, 0x06]);
        assert_eq!(zip[zip.len() - 12], 2); // entry count
    }
}
