// Runs a long-lived ffmpeg process that writes one lossless H.265 stream per camera.
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};

use anyhow::{Context, Result, bail};

use super::shared::{
    ensure_ffmpeg_lossless_hevc_support, format_ffmpeg_stderr, sanitize,
};

// Stores the output details from one H.265 recording session.
#[derive(Debug, Clone)]
pub struct H265SessionSummary {
    pub h265_path: PathBuf,
    pub frames_written: u64,
    pub width: u32,
    pub height: u32,
    pub frame_rate_hz: f64,
}

// Keeps one ffmpeg process alive while frames are being encoded.
pub struct H265CameraEncoder {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    h265_path: PathBuf,
    width: u32,
    height: u32,
    frame_rate_hz: f64,
    frame_index: u64,
}

impl H265CameraEncoder {
    /// Starts the ffmpeg encoder for one camera stream.
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

        let h265_path = output_dir.join(format!("{}_recording.h265", sanitize(camera_id)));
        let mut command = Command::new("ffmpeg");
        command
            .arg("-hide_banner")
            .arg("-loglevel")
            .arg("error")
            .arg("-nostats")
            .arg("-f")
            .arg("rawvideo")
            .arg("-pixel_format")
            .arg("gray")
            .arg("-video_size")
            .arg(format!("{}x{}", width, height))
            .arg("-framerate")
            .arg(format!("{}", frame_rate_hz))
            .arg("-i")
            .arg("pipe:0")
            .arg("-an")
            .arg("-c:v")
            .arg("libx265")
            .arg("-preset")
            .arg("ultrafast")
            .arg("-x265-params")
            .arg("lossless=1")
            .arg("-pix_fmt")
            .arg("gray")
            .arg("-f")
            .arg("hevc")
            .arg(&h265_path);

        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .with_context(|| {
                "spawn ffmpeg for lossless H.265 encoding; install ffmpeg and make sure it is on PATH"
            })?;

        let stdin = child
            .stdin
            .take()
            .context("take ffmpeg stdin for lossless H.265 encoder")?;

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

    // Writes one raw frame into the running ffmpeg process.
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
            .context("lossless encoder stdin already closed")?;
        stdin
            .write_all(raw_bytes)
            .context("write raw frame bytes into ffmpeg H.265 encoder")?;

        self.frame_index += 1;
        Ok(())
    }

    // Closes ffmpeg and returns the recording summary.
    pub fn finish(mut self) -> Result<H265SessionSummary> {
        drop(self.stdin.take());
        let output = self
            .child
            .take()
            .context("encoder process already closed")?
            .wait_with_output()
            .context("wait for ffmpeg H.265 encoder to exit")?;
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !output.status.success() {
            bail!(
                "ffmpeg H.265 encode failed{}",
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
    /// Stops the ffmpeg child process if the encoder is dropped early.
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
