//! Minimal EXIF parser focused on the Orientation tag.
//!
//! Strict enough to read the TIFF-formatted EXIF blob attached to
//! PNG, JPEG, and WebP images and report the
//! [Orientation](crate::Orientation) tag value (`0x0112`). Anything
//! else returns `None` rather than erroring.

const EXIF_PREFIX: &[u8] = b"Exif\0\0";
const ORIENTATION_TAG: u16 = 0x0112;
const IFD_ENTRY_BYTES: usize = 12;

/// Read the Orientation tag value from an EXIF blob, if present.
///
/// Returns the raw tag value (1..=8 are valid EXIF orientations,
/// other values are reported as-is and treated as
/// [`Orientation::Normal`](crate::Orientation::Normal) downstream).
pub(crate) fn parse_orientation(blob: &[u8]) -> Option<u16> {
    let tiff = strip_exif_prefix(blob);
    if tiff.len() < 8 {
        return None;
    }
    let big_endian = match &tiff[0..4] {
        b"MM\0*" => true,
        b"II*\0" => false,
        _ => return None,
    };
    let ifd_offset = read_u32(&tiff[4..8], big_endian) as usize;
    let ifd = tiff.get(ifd_offset..)?;
    if ifd.len() < 2 {
        return None;
    }
    let num_entries = read_u16(&ifd[..2], big_endian) as usize;
    let entries_start = ifd_offset.checked_add(2)?;
    let entries_end = entries_start.checked_add(num_entries.checked_mul(IFD_ENTRY_BYTES)?)?;
    let entries = tiff.get(entries_start..entries_end)?;

    for i in 0..num_entries {
        let off = i * IFD_ENTRY_BYTES;
        let entry = &entries[off..off + IFD_ENTRY_BYTES];
        let tag = read_u16(&entry[0..2], big_endian);
        if tag != ORIENTATION_TAG {
            continue;
        }
        // SHORT type (3) with count 1; value sits in the first two
        // bytes of the value/offset field at entry[8..10].
        return Some(read_u16(&entry[8..10], big_endian));
    }
    None
}

fn strip_exif_prefix(blob: &[u8]) -> &[u8] {
    if blob.starts_with(EXIF_PREFIX) {
        &blob[EXIF_PREFIX.len()..]
    } else {
        blob
    }
}

fn read_u16(b: &[u8], big_endian: bool) -> u16 {
    let arr = [b[0], b[1]];
    if big_endian {
        u16::from_be_bytes(arr)
    } else {
        u16::from_le_bytes(arr)
    }
}

fn read_u32(b: &[u8], big_endian: bool) -> u32 {
    let arr = [b[0], b[1], b[2], b[3]];
    if big_endian {
        u32::from_be_bytes(arr)
    } else {
        u32::from_le_bytes(arr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_tiff_with_orientation(big_endian: bool, value: u16) -> Vec<u8> {
        let mut blob = Vec::new();
        if big_endian {
            blob.extend_from_slice(b"MM\0*");
            blob.extend_from_slice(&8_u32.to_be_bytes());
        } else {
            blob.extend_from_slice(b"II*\0");
            blob.extend_from_slice(&8_u32.to_le_bytes());
        }
        // IFD starts at byte 8: 2-byte entry count, then entries.
        let count: u16 = 1;
        if big_endian {
            blob.extend_from_slice(&count.to_be_bytes());
            blob.extend_from_slice(&ORIENTATION_TAG.to_be_bytes());
            blob.extend_from_slice(&3_u16.to_be_bytes()); // type SHORT
            blob.extend_from_slice(&1_u32.to_be_bytes()); // count
            blob.extend_from_slice(&value.to_be_bytes());
            blob.extend_from_slice(&0_u16.to_be_bytes()); // padding
        } else {
            blob.extend_from_slice(&count.to_le_bytes());
            blob.extend_from_slice(&ORIENTATION_TAG.to_le_bytes());
            blob.extend_from_slice(&3_u16.to_le_bytes());
            blob.extend_from_slice(&1_u32.to_le_bytes());
            blob.extend_from_slice(&value.to_le_bytes());
            blob.extend_from_slice(&0_u16.to_le_bytes());
        }
        blob
    }

    #[test]
    fn reads_orientation_little_endian() {
        let blob = build_tiff_with_orientation(false, 6);
        assert_eq!(parse_orientation(&blob), Some(6));
    }

    #[test]
    fn reads_orientation_big_endian() {
        let blob = build_tiff_with_orientation(true, 3);
        assert_eq!(parse_orientation(&blob), Some(3));
    }

    #[test]
    fn handles_exif_prefix() {
        let mut blob = b"Exif\0\0".to_vec();
        blob.extend_from_slice(&build_tiff_with_orientation(false, 8));
        assert_eq!(parse_orientation(&blob), Some(8));
    }

    #[test]
    fn no_orientation_tag_returns_none() {
        let mut blob = Vec::new();
        blob.extend_from_slice(b"II*\0");
        blob.extend_from_slice(&8_u32.to_le_bytes());
        blob.extend_from_slice(&0_u16.to_le_bytes()); // zero entries
        assert_eq!(parse_orientation(&blob), None);
    }

    #[test]
    fn rejects_bogus_byte_order() {
        let blob = b"XX*\0\x08\0\0\0";
        assert_eq!(parse_orientation(blob), None);
    }

    #[test]
    fn empty_input_returns_none() {
        assert_eq!(parse_orientation(b""), None);
        assert_eq!(parse_orientation(b"Exif\0\0"), None);
    }

    #[test]
    fn truncated_ifd_returns_none() {
        let mut blob = Vec::new();
        blob.extend_from_slice(b"II*\0");
        blob.extend_from_slice(&8_u32.to_le_bytes());
        // No IFD bytes at offset 8.
        assert_eq!(parse_orientation(&blob), None);
    }
}
