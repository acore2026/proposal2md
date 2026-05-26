use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;

use proposal2md::{convert, ConvertOptions};

#[derive(Debug, Parser)]
#[command(author, version, about = "Convert 3GPP DOCX proposals to Markdown")]
struct Args {
    /// Input DOCX file or directory containing DOCX files.
    input: PathBuf,

    /// Output directory, or a .md file when converting a single input file.
    #[arg(short = 'o', long = "output")]
    output: Option<PathBuf>,

    /// Overwrite existing Markdown, report, and asset outputs.
    #[arg(long)]
    overwrite: bool,

    /// Return an error if unsupported figures or conversion warnings are found.
    #[arg(long)]
    strict: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let reports = convert(ConvertOptions {
        input: args.input,
        output: args.output,
        overwrite: args.overwrite,
        strict: args.strict,
    })?;

    for report in reports {
        println!("{} -> {}", report.source_path, report.output_path);
    }

    Ok(())
}
