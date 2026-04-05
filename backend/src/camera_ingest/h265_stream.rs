use std::env;
use std::fs::{self, File};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};

use anyhow::{Context, Result, bail};
use scuffle_h265::SpsNALUnit;

pub const FFMPEG_BIN_ENV: &str = "MJOLNIR_FFMPEG_BIN";

#[derive(Debug, Clone)]
pub struct H265SessionSummary {
    pub h265_path: PathBuf,
    pub frames_written: u64,
    pub width: u32,
    pub height: u32,
    pub frame_rate_hz: f64,
}

#[derive(Debug, Clone)]
pub struct RecoverySummary {
    pub output_dir: PathBuf,
    pub frames_recovered: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

pub struct H265CameraEncoder {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    pub h265_path: PathBuf,
    width: u32,
    height: u32,
    frame_rate_hz: f64,
    frame_index: u64,
}

impl H265CameraEncoder {
    pub fn new(
        output_dir: &Path,
        camera_id: &str,
        width: u32,
        height: u32,
        frame_rate_hz: f64,
    ) -> Result<Self> {
        ensure_ffmpeg_lossless_hevc_support()?;
        fs::create_dir_all(output_dir)
            .with_context(|| format!("create dir {}", output_dir.display()))?;

        let h265_path = output_dir.join(format!("{}.h265", sanitize(camera_id)));
        let ffmpeg = ffmpeg_program();
        // Spawn ffmpeg once and keep it alive for the whole session so raw MONO_8 frames stream
        // directly into libx265 instead of paying per-frame process startup cost.
        let mut child = Command::new(&ffmpeg)
            .arg("-hide_banner")
            .arg("-loglevel")
            .arg("error")
            .arg("-nostats")
            .arg("-f")
            .arg("rawvideo")
            .arg("-pixel_format")
            .arg("gray")
            .arg("-video_size")
            .arg(format!("{width}x{height}"))
            .arg("-framerate")
            .arg(format!("{frame_rate_hz}"))
            .arg("-i")
            .arg("pipe:0")
            .arg("-an")
            .arg("-c:v")
            // Encode with libx265 as raw Annex-B HEVC.
            .arg("libx265")
            .arg("-preset")
            .arg("ultrafast")
            .arg("-x265-params")
            // `lossless=1` is the switch that makes the H.265 stream bit-exact on decode.
            .arg("lossless=1")
            .arg("-pix_fmt")
            .arg("gray")
            .arg("-f")
            .arg("hevc")
            .arg("-y")
            .arg(&h265_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| {
                format!(
                    "spawn {ffmpeg} for lossless H.265 encoding; set {FFMPEG_BIN_ENV} if ffmpeg is not on PATH"
                )
            })?;

        let stdin = child
            .stdin
            .take()
            .context("take ffmpeg stdin for H.265 encoder")?;

        Ok(Self {
            child: Some(child),
            stdin: Some(stdin),
            h265_path,
            width,
            height,
            frame_rate_hz,
            frame_index: 0,
        })
    }

    pub fn push_frame(&mut self, raw_bytes: &[u8]) -> Result<()> {
        let expected = (self.width * self.height) as usize;
        if raw_bytes.len() != expected {
            bail!(
                "frame size mismatch: got {} bytes, expected {} ({}x{})",
                raw_bytes.len(),
                expected,
                self.width,
                self.height
            );
        }

        let stdin = self
            .stdin
            .as_mut()
            .context("encoder stdin already closed")?;
        // Push the raw grayscale bytes for this frame into ffmpeg's stdin.
        stdin
            .write_all(raw_bytes)
            .context("write raw frame bytes into ffmpeg encoder")?;

        self.frame_index += 1;
        Ok(())
    }

    pub fn finish(mut self) -> Result<H265SessionSummary> {
        drop(self.stdin.take());
        let output = self
            .child
            .take()
            .context("encoder process already closed")?
            .wait_with_output()
            .context("wait for ffmpeg lossless H.265 encoder to exit")?;
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() {
            bail!(
                "ffmpeg lossless H.265 encode failed for {}{}",
                self.h265_path.display(),
                format_ffmpeg_stderr(stderr.trim())
            );
        }

        Ok(H265SessionSummary {
            h265_path: self.h265_path.clone(),
            frames_written: self.frame_index,
            width: self.width,
            height: self.height,
            frame_rate_hz: self.frame_rate_hz,
        })
    }
}

impl Drop for H265CameraEncoder {
    fn drop(&mut self) {
        drop(self.stdin.take());

        if let Some(child) = self.child.as_mut() {
            if let Ok(None) = child.try_wait() {
                let _ = child.kill();
                let _ = child.wait();
            }
        }
    }
}

pub fn ensure_ffmpeg_available() -> Result<()> {
    let ffmpeg = ffmpeg_program();
    let output = Command::new(&ffmpeg)
        .arg("-version")
        .output()
        .with_context(|| {
            format!(
                "run {ffmpeg} -version; install ffmpeg or set {FFMPEG_BIN_ENV} to the ffmpeg executable"
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "ffmpeg is not usable{}",
            format_ffmpeg_stderr(stderr.trim())
        );
    }

    Ok(())
}

pub fn ensure_ffmpeg_lossless_hevc_support() -> Result<()> {
    ensure_ffmpeg_available()?;

    let ffmpeg = ffmpeg_program();
    let output = Command::new(&ffmpeg)
        .arg("-hide_banner")
        .arg("-encoders")
        .output()
        .context("inspect ffmpeg encoders")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "failed to inspect ffmpeg encoders{}",
            format_ffmpeg_stderr(stderr.trim())
        );
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.contains("libx265") {
        bail!(
            "ffmpeg is available but libx265 is not enabled; install/build ffmpeg with libx265 support"
        );
    }

    Ok(())
}

pub fn recover_h265_to_png(h265_path: &Path, output_dir: &Path) -> Result<RecoverySummary> {
    ensure_ffmpeg_available()?;
    prepare_output_dir(output_dir)?;

    // Inspect the SPS with scuffle-h265 before decode so we can validate the stream shape.
    let sps_dimensions = match inspect_h265_sps(h265_path) {
        Ok(sps) => {
            let width = u32::try_from(sps.rbsp.cropped_width())
                .context("cropped width from SPS does not fit into u32")?;
            let height = u32::try_from(sps.rbsp.cropped_height())
                .context("cropped height from SPS does not fit into u32")?;
            let bit_depth = sps.rbsp.bit_depth_y();

            if bit_depth != 8 {
                bail!(
                    "only 8-bit MONO_8 streams are supported for PNG recovery, found SPS bit depth {}",
                    bit_depth
                );
            }

            Some((width, height))
        }
        Err(error) => {
            eprintln!(
                "Warning: could not parse SPS from {} via scuffle-h265: {error:#}. Continuing with ffmpeg decode.",
                h265_path.display()
            );
            None
        }
    };

    let ffmpeg = ffmpeg_program();
    // ffmpeg writes the recovered frames as frame_000000.png, frame_000001.png, ...
    let output_pattern = output_dir.join("frame_%06d.png");
    let output = Command::new(&ffmpeg)
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-nostats")
        .arg("-i")
        .arg(h265_path)
        .arg("-vsync")
        .arg("0")
        .arg("-start_number")
        .arg("0")
        .arg("-pix_fmt")
        .arg("gray")
        .arg("-y")
        .arg(&output_pattern)
        .output()
        .with_context(|| {
            format!(
                "spawn {ffmpeg} for H.265 recovery; set {FFMPEG_BIN_ENV} if ffmpeg is not on PATH"
            )
        })?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        bail!(
            "ffmpeg H.265 recovery failed for {}{}",
            h265_path.display(),
            format_ffmpeg_stderr(stderr.trim())
        );
    }

    let frames_recovered = count_recovered_pngs(output_dir)?;
    if frames_recovered == 0 {
        bail!(
            "ffmpeg completed recovery for {} but wrote no PNG frames into {}",
            h265_path.display(),
            output_dir.display()
        );
    }

    Ok(RecoverySummary {
        output_dir: output_dir.to_path_buf(),
        frames_recovered,
        width: sps_dimensions.map(|(width, _)| width),
        height: sps_dimensions.map(|(_, height)| height),
    })
}

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

pub fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

fn ffmpeg_program() -> String {
    env::var(FFMPEG_BIN_ENV).unwrap_or_else(|_| "ffmpeg".to_string())
}

fn prepare_output_dir(output_dir: &Path) -> Result<()> {
    if output_dir.exists() {
        let mut entries = fs::read_dir(output_dir)
            .with_context(|| format!("read output dir {}", output_dir.display()))?;
        if entries
            .next()
            .transpose()
            .with_context(|| format!("read output dir {}", output_dir.display()))?
            .is_some()
        {
            bail!(
                "output directory {} must be empty or absent before recovery",
                output_dir.display()
            );
        }
    } else {
        fs::create_dir_all(output_dir)
            .with_context(|| format!("create output dir {}", output_dir.display()))?;
    }

    Ok(())
}

fn count_recovered_pngs(output_dir: &Path) -> Result<u64> {
    let mut count = 0u64;

    for entry in
        fs::read_dir(output_dir).with_context(|| format!("read dir {}", output_dir.display()))?
    {
        let entry = entry.with_context(|| format!("read dir {}", output_dir.display()))?;
        let path = entry.path();
        let is_frame_png = path.extension().is_some_and(|ext| ext == "png")
            && path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .is_some_and(|stem| stem.starts_with("frame_"));

        if is_frame_png {
            count += 1;
        }
    }

    Ok(count)
}

fn format_ffmpeg_stderr(stderr: &str) -> String {
    if stderr.is_empty() {
        String::new()
    } else {
        format!(" ({stderr})")
    }
}

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
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        count_recovered_pngs, find_annex_b_start_code, find_sps_nalu_payload, prepare_output_dir,
        sanitize,
    };

    fn temp_test_dir(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before unix epoch")
            .as_nanos();
        env::temp_dir().join(format!(
            "mjolnir_h265_stream_tests_{name}_{}_{}",
            std::process::id(),
            unique
        ))
    }

    #[test]
    fn sanitize_replaces_unsupported_characters() {
        assert_eq!(sanitize("Cam/01:mono"), "Cam_01_mono");
    }

    #[test]
    fn finds_annex_b_start_codes() {
        let bytes = [9, 0, 0, 1, 0x42, 1, 2, 0, 0, 0, 1, 0x44];
        assert_eq!(find_annex_b_start_code(&bytes, 0), Some((1, 3)));
        assert_eq!(find_annex_b_start_code(&bytes, 5), Some((7, 4)));
    }

    #[test]
    fn extracts_only_the_sps_nalu() {
        let bytes = [
            0, 0, 0, 1, 0x40, 0x01, 0xaa, 0xbb, 0, 0, 1, 0x42, 0x01, 0xcc, 0xdd, 0, 0, 1, 0x44,
            0x01, 0xee,
        ];

        assert_eq!(find_sps_nalu_payload(&bytes), Some(&bytes[11..15]));
    }

    #[test]
    fn prepare_output_dir_creates_missing_directory() {
        let dir = temp_test_dir("create");
        assert!(!dir.exists());

        prepare_output_dir(&dir).expect("should create missing output dir");
        assert!(dir.exists());

        fs::remove_dir_all(&dir).expect("cleanup temp dir");
    }

    #[test]
    fn prepare_output_dir_rejects_non_empty_directory() {
        let dir = temp_test_dir("reject_non_empty");
        fs::create_dir_all(&dir).expect("create temp dir");
        fs::write(dir.join("existing.txt"), b"data").expect("seed temp dir");

        let error = prepare_output_dir(&dir).expect_err("non-empty dir should be rejected");
        assert!(
            error
                .to_string()
                .contains("must be empty or absent before recovery")
        );

        fs::remove_dir_all(&dir).expect("cleanup temp dir");
    }

    #[test]
    fn count_recovered_pngs_only_counts_frame_pngs() {
        let dir = temp_test_dir("count");
        fs::create_dir_all(&dir).expect("create temp dir");
        fs::write(dir.join("frame_000000.png"), b"a").expect("write frame png");
        fs::write(dir.join("frame_000001.png"), b"b").expect("write frame png");
        fs::write(dir.join("notes.txt"), b"skip").expect("write notes file");
        fs::write(dir.join("preview.png"), b"skip").expect("write non-frame png");

        assert_eq!(count_recovered_pngs(&dir).expect("count pngs"), 2);

        fs::remove_dir_all(&dir).expect("cleanup temp dir");
    }
}
