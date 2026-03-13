use std::env;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result, bail};

fn main() -> Result<()> {
    let mut args = env::args().skip(1);
    let input = args
        .next()
        .map(PathBuf::from)
        .context("usage: cargo run -p ttl2ofn -- <input.ttl> <output.ofn>")?;
    let output = args
        .next()
        .map(PathBuf::from)
        .context("usage: cargo run -p ttl2ofn -- <input.ttl> <output.ofn>")?;

    if args.next().is_some() {
        bail!("usage: cargo run -p ttl2ofn -- <input.ttl> <output.ofn>");
    }

    let ofn = ttl2ofn::convert_file(&input)
        .with_context(|| format!("failed to convert {}", input.display()))?;
    fs::write(&output, ofn).with_context(|| format!("failed to write {}", output.display()))?;
    println!("wrote {}", output.display());
    Ok(())
}
