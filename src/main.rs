use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;

use perflab::{RunConfig, run};

#[derive(Parser)]
struct PerfLab {
    /// Number of warm-up runs to discard
    #[arg(short = 'w', long = "warmup", default_value_t = 2)]
    warmup: u8,

    /// Number of measured runs
    #[arg(short = 'r', long = "runs", default_value_t = 14)]
    runs: u16,

    /// Directory where the results are written
    #[arg(long, default_value = "results")]
    out: PathBuf,

    /// The command to measure, after --
    #[arg(last = true, required = true, num_args = 1..)]
    command: Vec<String>,
    #[arg(long)]
    core: Option<u32>,
}

fn main() -> Result<()> {
    let cli = PerfLab::parse();

    let config = RunConfig {
        command: cli.command,
        warmup: cli.warmup,
        runs: cli.runs,
        out_dir: cli.out,
        core: cli.core,
    };

    let outputs = run(&config)?;
    println!("wrote {}", outputs.json.display());
    println!("wrote {}", outputs.markdown.display());
    Ok(())
}
