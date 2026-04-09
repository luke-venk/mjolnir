// Holds shared ffmpeg checks and string helpers for the compression module.

use std::process::Command;

use anyhow::{Context, Result, bail};

// Verifies that ffmpeg supports the libx265 encoder.
pub fn ensure_ffmpeg_lossless_hevc_support() -> Result<()> {
    let output = Command::new("ffmpeg")
        .arg("-hide_banner")
        .arg("-encoders")
        .output()
        .context("inspect ffmpeg encoders; install ffmpeg and make sure it is on PATH")?;

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

// Replaces unsupported characters in a file-system name.
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

// Formats ffmpeg stderr for inclusion in error messages.
pub(super) fn format_ffmpeg_stderr(stderr: &str) -> String {
    if stderr.is_empty() {
        String::new()
    } else {
        format!(" ({stderr})")
    }
}

#[cfg(test)]
mod tests {
    use super::sanitize;

    /// Confirms that unsupported path characters are replaced.
    #[test]
    fn sanitize_replaces_unsupported_characters() {
        assert_eq!(sanitize("Cam/01:mono"), "Cam_01_mono");
    }
}