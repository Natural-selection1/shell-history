//! Dump a zsh history file to stdout, one entry per line.
//!
//! Usage: cargo run --example dump_zsh -- ~/.zsh_history

use clap::Parser;
use std::io::BufReader;
use std::{env, fs::File, io, path::PathBuf};
use zsh_history::ParseOutcome;
use zsh_history::ZshParser;

/// Print Zsh history entries with their timestamps.
#[derive(Parser)]
struct Args {
    /// Zsh history file to read (defaults to ~/.zsh_history).
    input: Option<PathBuf>,
}

fn main() -> io::Result<()> {
    let path = match Args::parse().input {
        Some(path) => path,
        None => PathBuf::from(
            env::var_os("HOME")
                .ok_or_else(|| io::Error::other("pass an input path when HOME is unset"))?,
        )
        .join(".zsh_history"),
    };
    let f = BufReader::new(File::open(path)?);
    let mut entries = 0usize;
    let mut skipped = 0usize;
    for outcome in ZshParser::new().parse(f) {
        match outcome {
            ParseOutcome::Entry(e) => {
                entries += 1;
                let ts = e.timing.timestamp;
                let cmd = e.command_lossy().replace('\n', "⏎");
                println!("[{ts}] {cmd}");
            }
            ParseOutcome::Skipped { reason, .. } => {
                skipped += 1;
                eprintln!("skipped: {reason}");
            }
        }
    }
    eprintln!("--- {entries} entries, {skipped} skipped ---");
    Ok(())
}
