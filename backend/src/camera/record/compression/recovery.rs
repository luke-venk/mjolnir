// Decodes saved H.265 recordings back into PNG frames.

use std::env;
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, bail};

use super::inspect::inspect_h265_sps;
use super::shared::format_ffmpeg_stderr;

// Stores the output details from one PNG recovery run.
#[derive(Debug, Clone)]
pub struct RecoverySummary {
    pub output_dir: PathBuf,
    pub frames_recovered: u64,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

// Decodes a directory of H.265 files into PNG files.
pub fn recover_h265_dir_to_pngs(input_dir: &Path, output_dir: &Path) -> Result<RecoverySummary> {
    prepare_output_dir(output_dir)?;

    let mut frame_paths = Vec::new();
    for entry in fs::read_dir(input_dir)
        .with_context(|| format!("read input dir {}", input_dir.display()))?
    {
        let entry = entry.with_context(|| format!("read input dir {}", input_dir.display()))?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "h265") {
            frame_paths.push(path);
        }
    }
    frame_paths.sort();

    if frame_paths.is_empty() {
        bail!(
            "input directory {} does not contain any .h265 frame files",
            input_dir.display()
        );
    }

    let merged_stream_path = concat_h265_frames_to_temp_stream(&frame_paths)?;
    let recovery = recover_h265_to_png(&merged_stream_path, output_dir);
    let _ = fs::remove_file(&merged_stream_path);
    recovery
}

// Decodes one H.265 stream file into PNG frames.
pub fn recover_h265_to_png(h265_path: &Path, output_dir: &Path) -> Result<RecoverySummary> {
    prepare_output_dir(output_dir)?;
    let existing_frames = count_recovered_pngs(output_dir)?;
    let start_number = next_available_frame_index(output_dir)?;

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

    let output_pattern = output_dir.join("frame_%06d.png");
    let output = std::process::Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-loglevel")
        .arg("error")
        .arg("-nostats")
        .arg("-i")
        .arg(h265_path)
        .arg("-vsync")
        .arg("0")
        .arg("-start_number")
        .arg(start_number.to_string())
        .arg("-pix_fmt")
        .arg("gray")
        .arg("-y")
        .arg(&output_pattern)
        .output()
        .context("spawn ffmpeg for H.265 recovery; install ffmpeg and make sure it is on PATH")?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        bail!(
            "ffmpeg H.265 recovery failed for {}{}",
            h265_path.display(),
            format_ffmpeg_stderr(stderr.trim())
        );
    }

    let total_frames = count_recovered_pngs(output_dir)?;
    let frames_recovered = total_frames.saturating_sub(existing_frames);
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

// Creates the recovery output directory if it does not exist.
fn prepare_output_dir(output_dir: &Path) -> Result<()> {
    if !output_dir.exists() {
        fs::create_dir_all(output_dir)
            .with_context(|| format!("create output dir {}", output_dir.display()))?;
    }

    Ok(())
}

// Counts PNG files that match the recovered frame naming pattern.
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

// Finds the next frame index available in an output directory.
fn next_available_frame_index(output_dir: &Path) -> Result<u64> {
    let mut next_index = 0u64;

    for entry in
        fs::read_dir(output_dir).with_context(|| format!("read dir {}", output_dir.display()))?
    {
        let entry = entry.with_context(|| format!("read dir {}", output_dir.display()))?;
        let path = entry.path();
        let Some("png") = path.extension().and_then(|ext| ext.to_str()) else {
            continue;
        };
        let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let Some(index_str) = stem.strip_prefix("frame_") else {
            continue;
        };
        let Ok(index) = index_str.parse::<u64>() else {
            continue;
        };
        next_index = next_index.max(index.saturating_add(1));
    }

    Ok(next_index)
}

// Builds a temporary file path for a merged H.265 stream.
fn temp_concat_stream_path() -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock before unix epoch")
        .as_nanos();
    env::temp_dir().join(format!(
        "mjolnir_recover_concat_{}_{}.h265",
        std::process::id(),
        unique
    ))
}

// Concatenates multiple H.265 files into one temporary stream file.
fn concat_h265_frames_to_temp_stream(frame_paths: &[PathBuf]) -> Result<PathBuf> {
    let temp_path = temp_concat_stream_path();
    let temp_file = File::create(&temp_path)
        .with_context(|| format!("create temp stream {}", temp_path.display()))?;
    let mut writer = BufWriter::new(temp_file);

    for frame_path in frame_paths {
        let mut frame_file = File::open(frame_path)
            .with_context(|| format!("open frame file {}", frame_path.display()))?;
        std::io::copy(&mut frame_file, &mut writer)
            .with_context(|| format!("append frame file {}", frame_path.display()))?;
    }

    writer
        .flush()
        .with_context(|| format!("flush temp stream {}", temp_path.display()))?;

    Ok(temp_path)
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        count_recovered_pngs, next_available_frame_index, prepare_output_dir,
        recover_h265_dir_to_pngs,
    };

    fn temp_test_dir(name: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before unix epoch")
            .as_nanos();
        env::temp_dir().join(format!(
            "mjolnir_h265_recovery_tests_{name}_{}_{}",
            std::process::id(),
            unique
        ))
    }

    /// Confirms that a missing recovery directory is created.
    #[test]
    fn prepare_output_dir_creates_missing_directory() {
        let dir = temp_test_dir("create");
        assert!(!dir.exists());

        prepare_output_dir(&dir).expect("should create missing output dir");
        assert!(dir.exists());

        fs::remove_dir_all(&dir).expect("cleanup temp dir");
    }

    /// Confirms that an existing non-empty recovery directory is allowed.
    #[test]
    fn prepare_output_dir_allows_non_empty_directory() {
        let dir = temp_test_dir("allow_non_empty");
        fs::create_dir_all(&dir).expect("create temp dir");
        fs::write(dir.join("existing.txt"), b"data").expect("seed temp dir");

        prepare_output_dir(&dir).expect("non-empty dir should still be accepted");
        assert!(dir.exists());

        fs::remove_dir_all(&dir).expect("cleanup temp dir");
    }

    /// Confirms that only recovered frame PNGs are counted.
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

    /// Confirms that the next frame index skips existing PNG files.
    #[test]
    fn next_available_frame_index_skips_existing_frames() {
        let dir = temp_test_dir("next_index");
        fs::create_dir_all(&dir).expect("create temp dir");
        fs::write(dir.join("frame_000000.png"), b"a").expect("write first frame png");
        fs::write(dir.join("frame_000007.png"), b"b").expect("write later frame png");
        fs::write(dir.join("notes.txt"), b"skip").expect("write notes file");

        assert_eq!(next_available_frame_index(&dir).expect("find next frame index"), 8);

        fs::remove_dir_all(&dir).expect("cleanup temp dir");
    }

    /// Confirms that an empty H.265 input directory is rejected.
    #[test]
    fn recover_h265_dir_to_pngs_rejects_empty_directory() {
        let input_dir = temp_test_dir("empty_h265_dir_input");
        let output_dir = temp_test_dir("empty_h265_dir_output");
        fs::create_dir_all(&input_dir).expect("create input dir");

        let error = recover_h265_dir_to_pngs(&input_dir, &output_dir)
            .expect_err("empty h265 dir should be rejected");
        assert!(error.to_string().contains("does not contain any .h265 frame files"));

        fs::remove_dir_all(&input_dir).expect("cleanup input dir");
        if output_dir.exists() {
            fs::remove_dir_all(&output_dir).expect("cleanup output dir");
        }
    }
}
