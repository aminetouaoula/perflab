use anyhow::{Context, Result, bail};
use std::process::{Command, Stdio};
use wait4::Wait4;
pub fn mem_usage_reader(args: &[String]) -> Result<u64> {
    let (program, rest) = args.split_first().context("no command given to measure")?;
    let child = Command::new(program)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .args(rest)
        .spawn()
        .with_context(|| format!("failed to start {program}"))?;
    let result = child.wait4()?;
    if !result.status.success() {
        bail!("{program} exited with {}", result.status);
    }

    Ok(result.rusage.maxrss)
}
