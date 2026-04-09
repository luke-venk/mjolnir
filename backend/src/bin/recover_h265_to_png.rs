use std::path::PathBuf;

use anyhow::Result;
use backend_lib::camera::record::compression::recover_compressed_dir_to_pngs;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "recover_h265_to_png")]
#[command(about = "Recovers saved compressed camera frames back into PNG frames.")]
struct RecoverH265Args {
    #[arg(long)]
    input_path: String,

    #[arg(long)]
    output_dir: String,
}

fn run() -> Result<()> {
    let args = RecoverH265Args::parse();
    let input_path = PathBuf::from(&args.input_path);
    let output_dir = PathBuf::from(&args.output_dir);

    if !input_path.is_dir() {
        anyhow::bail!(
            "input path {} must be a directory of compressed frame files",
            input_path.display()
        );
    }

    let frames_recovered = recover_compressed_dir_to_pngs(&input_path, &output_dir)?;
    println!(
        "Recovered {} frame(s) into {}",
        frames_recovered,
        output_dir.display()
    );

    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("recover_h265_to_png failed: {error:#}");
        std::process::exit(1);
    }
}
