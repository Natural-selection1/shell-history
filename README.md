# zsh-history

Streaming, byte-preserving parser for **Zsh** history files. It supports plain
history, `EXTENDED_HISTORY` and files containing both formats, including metafy
encoding, multiline commands, escaped leading colons and trailing backslashes.

Commands remain bytes, including non-UTF-8 content. Parsed entries also retain
**exact original record bytes**, including headers, escapes and line terminators.
Parsed entries expose the time Zsh assigns on import and whether the start time
came from the file. See [reader semantics](DIFFERENCES.md) for details.

## Parse and filter without re-encoding

```rust,no_run
use zsh_history::{ZshParser, ParseOutcome};
use std::{fs::File, io::{self, BufReader, Write}};

fn main() -> io::Result<()> {
    let path = std::path::PathBuf::from(std::env::var_os("HOME").expect("HOME"))
        .join(".zsh_history");
    let input = BufReader::new(File::open(path)?);
    let mut output = io::stdout().lock();

    for outcome in ZshParser::new().parse(input) {
        match outcome {
            ParseOutcome::Entry(entry) if entry.command.starts_with(b"git ") => {
                // Copy the original record, not the decoded command.
                output.write_all(&entry.source.raw)?;
            }
            ParseOutcome::Entry(_) => {}
            ParseOutcome::Skipped { source, reason } => {
                eprintln!("{:?}: {reason}", source.span);
            }
        }
    }
    Ok(())
}
```

`entry.command_str()` validates UTF-8 without copying. `entry.command_lossy()`
provides a display string; it must not be used to reconstruct source records.
The original `source.raw` does not change if a caller modifies `command`.
A writer for edited commands is outside this crate's current scope.

Examples:

```sh
cargo run --example dump_zsh -- ~/.zsh_history
cargo run --example clean_zsh -- ~/.zsh_history cleaned.hist y code
cargo run --example filter_zsh -- ~/.zsh_history 'git ' > filtered.hist
```

The clean_zsh example deduplicates commands and excludes exact matches (here,
`y` and `code`). It writes the retained raw records to a new file.

The filter example reports errors and returns a failure status if records were
skipped. Write to a **different file**: redirecting over the input would truncate
it before parsing begins. File locking and atomic replacement are caller concerns.

## Source bytes and errors

The parser reads one logical record at a time. For each parsed entry,
`source.raw` holds the exact input bytes and `source.span` gives their
half-open range in the stream. Filters can use `entry.command.len()` for the
decoded command length or `source.raw.len()` for the stored record length,
including its header and line terminators.

Empty commands are entries. A corrupt input segment reports `CorruptHistory`
and ends the import. Zsh can discard an unfinished EOF tail.
The parser retries interrupted reads. Other I/O failures yield one error outcome
and end iteration. The error outcome carries the bytes consumed for that record.

Use `parse()` to handle errors explicitly. `parse_vec()` intentionally discards
all skipped outcomes. Consumers need not collect an entire file into memory.
Source ranges are not stable references into files concurrently changed by a shell.

## API migration from the initial 0.1.0 implementation

This revision deliberately changes the public API:

- `parse_auto`, `detect_parser`, and `HistoryParser::detect` are removed.
  Callers explicitly construct the required shell parser.
- Import `ZshParser` from the crate root and pass a buffered reader directly to
  `parse` or `parse_vec`. `HistoryParser`, `Shell` and the entry's `shell` field
  have been removed.
- Every parsed entry has `Timing { timestamp, duration, from_file }`.
  A missing or zero start takes the import time. Use `from_file` to identify
  stored start times.
- The unused public `lines` module, `ByteLines` and `byte_lines` helper are
  removed. Physical-line scanning is an internal parser implementation detail.
- `HistoryEntry.command`: `String` → `Vec<u8>`; use `command_str()` or
  `command_lossy()` for text access.
- `source_offset` → `source.span.start`; the full raw record
  and range are available on every parsed entry. Obtain entries from the parser.
- `ParseOutcome::Skipped { raw, reason }` → `{ source, reason }`.
- `zsh::unmetafy` returns `Cow<'_, [u8]>` directly and retains an unpaired trailing escape as a literal byte.

## Validation

```sh
cargo test
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt -- --check
RUSTDOCFLAGS='-D warnings' cargo doc --no-deps --all-features
```

See [tests/README.md](tests/README.md) for the test layout and contribution rules.
Checked-in fixtures are mandatory, so ordinary tests need no installed shell.
They include actual shell-written records and length-prefixed binary read-back
oracles (little-endian `u64` length followed by command bytes). Tests compare
commands, timing, raw spans, small-buffer decoding and filtered-copy
round trips. `.gitattributes` prevents Git newline conversion of fixture bytes.

The three history formats share a fixed command sequence recorded with
[Zsh 5.9](tests/fixtures/zsh.version). The project maintains those expectations
alongside the parser.

## License

MIT
