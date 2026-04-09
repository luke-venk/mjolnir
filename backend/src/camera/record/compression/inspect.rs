// Reads H.265 headers so the recorder can inspect stream dimensions and format.

use std::fs::File;
use std::io::{Cursor, Read};
use std::path::Path;

use anyhow::{Context, Result};
use scuffle_h265::SpsNALUnit;

// Reads the SPS NAL unit from an H.265 file.
pub fn inspect_h265_sps(h265_path: &Path) -> Result<SpsNALUnit> {
    let mut file = File::open(h265_path)
        .with_context(|| format!("open {} for SPS inspection", h265_path.display()))?;
    let mut buf = vec![0u8; 65536];
    let bytes_read = file.read(&mut buf).context("read h265 header")?;
    buf.truncate(bytes_read);

    let nalu_payload = find_sps_nalu_payload(&buf)
        .context("could not find SPS NAL unit in first 64KiB of file")?;
    let mut reader = Cursor::new(nalu_payload);
    let sps = SpsNALUnit::parse(&mut reader).context("parse SPS NAL unit")?;

    Ok(sps)
}

// Finds the SPS payload in a chunk of Annex-B H.265 bytes.
fn find_sps_nalu_payload(buf: &[u8]) -> Option<&[u8]> {
    const SPS_HEADER_BYTE0: u8 = 0x42;
    let mut search_from = 0usize;

    while let Some((start_index, start_code_len)) = find_annex_b_start_code(buf, search_from) {
        let nal_start = start_index + start_code_len;
        if nal_start >= buf.len() {
            break;
        }

        let next_start = find_annex_b_start_code(buf, nal_start)
            .map(|(index, _)| index)
            .unwrap_or(buf.len());

        if buf[nal_start] == SPS_HEADER_BYTE0 {
            return Some(&buf[nal_start..next_start]);
        }

        search_from = next_start;
    }

    None
}

// Finds the next Annex-B start code in a byte slice.
fn find_annex_b_start_code(buf: &[u8], from: usize) -> Option<(usize, usize)> {
    if buf.len() < 3 || from >= buf.len().saturating_sub(2) {
        return None;
    }

    for index in from..=buf.len() - 3 {
        if index + 4 <= buf.len() && buf[index..index + 4] == [0, 0, 0, 1] {
            return Some((index, 4));
        }

        if buf[index..index + 3] == [0, 0, 1] {
            return Some((index, 3));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::{find_annex_b_start_code, find_sps_nalu_payload};

    // Confirms that Annex-B start codes are found at the right offsets.
    #[test]
    fn finds_annex_b_start_codes() {
        let bytes = [9, 0, 0, 1, 0x42, 1, 2, 0, 0, 0, 1, 0x44];
        assert_eq!(find_annex_b_start_code(&bytes, 0), Some((1, 3)));
        assert_eq!(find_annex_b_start_code(&bytes, 5), Some((7, 4)));
    }

    // Confirms that only the SPS payload is returned from a stream chunk.
    #[test]
    fn extracts_only_the_sps_nalu() {
        let bytes = [
            0, 0, 0, 1, 0x40, 0x01, 0xaa, 0xbb, 0, 0, 1, 0x42, 0x01, 0xcc, 0xdd, 0, 0, 1, 0x44,
            0x01, 0xee,
        ];

        assert_eq!(find_sps_nalu_payload(&bytes), Some(&bytes[11..15]));
    }
}
