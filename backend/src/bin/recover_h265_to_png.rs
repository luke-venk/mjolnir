use std::path::PathBuf;

use anyhow::Result;
use backend_lib::camera::record::compression::{recover_h265_dir_to_pngs, recover_h265_to_png};
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "recover_h265_to_png")]
#[command(about = "Decodes a lossless H.265 camera stream back into PNG frames.")]
struct RecoverH265Args {
    #[arg(long)]
    h265_path: String,

    #[arg(long)]
    output_dir: String,
}

fn run() -> Result<()> {
    let args = RecoverH265Args::parse();
    let h265_path = PathBuf::from(&args.h265_path);
    let output_dir = PathBuf::from(&args.output_dir);
    let summary = if h265_path.is_dir() {
        recover_h265_dir_to_pngs(&h265_path, &output_dir)?
    } else {
        recover_h265_to_png(&h265_path, &output_dir)?
    };

    match (summary.width, summary.height) {
        (Some(width), Some(height)) => println!(
            "Recovered {} frame(s) at {}x{} into {}",
            summary.frames_recovered,
            width,
            height,
            summary.output_dir.display()
        ),
        _ => println!(
            "Recovered {} frame(s) into {}",
            summary.frames_recovered,
            summary.output_dir.display()
        ),
    }

    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("recover_h265_to_png failed: {error:#}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::RecoverH265Args;
    use clap::Parser;

    #[test]
    fn cli_parses_required_paths() {
        let args = RecoverH265Args::try_parse_from([
            "recover_h265_to_png",
            "--h265-path",
            "/tmp/test.h265",
            "--output-dir",
            "/tmp/out",
        ])
        .expect("CLI args should parse");

        assert_eq!(args.h265_path, "/tmp/test.h265");
        assert_eq!(args.output_dir, "/tmp/out");
    }

    #[test]
    fn cli_requires_both_paths() {
        assert!(RecoverH265Args::try_parse_from(["recover_h265_to_png"]).is_err());
    }
}
