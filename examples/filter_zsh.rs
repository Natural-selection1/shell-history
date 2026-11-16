//! Copy Zsh records whose decoded command starts with a byte prefix.
//! Usage: cargo run --example filter_zsh -- ~/.zsh_history 'git ' > filtered.hist
//! Output must be a different file: redirecting over the input truncates it.
use clap::Parser;
use std::{
    fs::File,
    io::{self, BufReader, Write},
    path::PathBuf,
};
use zsh_history::{ParseOutcome, ZshParser};

/// Copy matching Zsh history records to standard output.
#[derive(Parser)]
struct Args {
    /// Zsh history file to read.
    input: PathBuf,
    /// UTF-8 prefix of the decoded command.
    prefix: String,
}

fn main() -> io::Result<()> {
    let Args { input, prefix } = Args::parse();
    let input = BufReader::new(File::open(input)?);
    let mut out = io::stdout().lock();
    let mut skipped = false;
    for result in ZshParser::new().parse(input) {
        match result {
            ParseOutcome::Entry(e) if e.command.starts_with(prefix.as_bytes()) => {
                out.write_all(&e.source.raw)?;
            }
            ParseOutcome::Entry(_) => {}
            ParseOutcome::Skipped { source, reason } => {
                skipped = true;
                eprintln!("{:?}: {reason}", source.span);
            }
        }
    }
    out.flush()?;
    if skipped {
        return Err(io::Error::other("some input records were skipped"));
    }
    Ok(())
}
