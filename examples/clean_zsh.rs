//! Usage: cargo run --example clean_zsh -- INPUT NEW_OUTPUT [EXACT_COMMAND ...]
//! Keep one copy per decoded command. A fully timestamped file uses start
//! times; mixed or plain history uses file order. Ties keep the later record.
use clap::Parser;
use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    fs::{File, OpenOptions},
    io::{self, BufReader, BufWriter, Write},
    path::PathBuf,
};
use zsh_history::{ParseOutcome, ZshParser};

/// Deduplicate Zsh history and exclude exact commands.
#[derive(Parser)]
struct Args {
    /// Zsh history file to read.
    input: PathBuf,
    /// New file for the retained records.
    new_output: PathBuf,
    /// Commands to exclude, matched byte for byte.
    exact_command: Vec<OsString>,
}

fn main() -> io::Result<()> {
    let args = Args::parse();
    let excluded: HashSet<_> = args
        .exact_command
        .into_iter()
        .map(OsString::into_encoded_bytes)
        .collect();

    let reader = BufReader::new(File::open(args.input)?);
    let mut entries = Vec::new();
    for outcome in ZshParser::new().parse(reader) {
        match outcome {
            ParseOutcome::Entry(entry) => entries.push(entry),
            ParseOutcome::Skipped { source, reason } => {
                return Err(io::Error::other(format!("{:?}: {reason}", source.span)));
            }
        }
    }

    let dated = entries.iter().all(|e| e.timing.from_file);
    let mut latest = HashMap::new();
    let mut removed = 0;
    for (index, entry) in entries.iter().enumerate() {
        if excluded.contains(&entry.command) {
            removed += 1;
            continue;
        }
        let rank = (dated.then_some(entry.timing.timestamp), index);
        let best = latest.entry(entry.command.as_slice()).or_insert(rank);
        *best = (*best).max(rank);
    }

    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    std::os::unix::fs::OpenOptionsExt::mode(&mut options, 0o600);
    let mut out = BufWriter::new(options.open(args.new_output)?);
    for (index, entry) in entries.iter().enumerate() {
        if latest
            .get(entry.command.as_slice())
            .is_some_and(|&(_, i)| i == index)
        {
            out.write_all(&entry.source.raw)?;
        }
    }
    out.flush()?;
    eprintln!(
        "parsed={}, excluded={removed}, duplicates={}, kept={}, order={}",
        entries.len(),
        entries.len() - removed - latest.len(),
        latest.len(),
        if dated { "timestamp" } else { "file" },
    );
    Ok(())
}
