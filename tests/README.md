# Testing the parser contracts

The suite separates shell-specific decoding from shared stream contracts and
fixed Zsh 5.9 samples. Ordinary `cargo test` needs no Python or installed
shells. Both default and `--all-features` configurations must pass because the
optional `memchr` feature changes the physical-line scanner.

| Location | Responsibility |
| --- | --- |
| `parser/zsh.rs` | Zsh escaping, file endings and continuation recovery |
| `parser/timing.rs` | Numeric prefixes, missing fields, elapsed time and read-time fallback |
| `parser/records.rs` | Metafy byte pairs, legacy encoding, EOF, NUL, and raw record boundaries |
| `parser/contracts.rs` | Command bytes, source coverage and buffer independence |
| `parser/io.rs` | Interrupted reads, terminal failures and consumed bytes |
| `parser/support.rs` | Shared public-API assertions, including exact raw bytes and contiguous source spans |
| `examples_test.rs` | Clean, filter and dump command lines, including raw output, ordering and file errors |
| `fixtures_test.rs` | Three offline comparisons against checked-in shell read-back samples |
| `zsh_5_9_baseline.rs` | Standalone port of the upstream history-file regression, with seven fixed command byte strings |

`parser_test.rs` is the integration-test entry point. The modules underneath it
test the public API.

## Adding regression coverage

- Add a focused case to the responsible module. For a bug fix, the input and
  expected output should demonstrate the original failure.
- Assert command bytes and relevant metadata or error variants. Use the shared
  parsing helper to also check raw bytes and source spans.
- For corrupt input, append a valid record and verify that iteration stops.
- Preserve byte strings in expectations; lossy text conversion can hide damage.
- Avoid allocator-capacity assertions, private algorithm details and timing
  thresholds. Test observable behavior and retained data instead.

The deterministic byte corpus uses a fixed seed and format-weighted punctuation.
It checks source partitioning and compares full outcomes across buffers of
1, 2, 7 and 8192 bytes. It is a reproducible invariant
check, not exhaustive fuzzing or a substitute for independently known commands.

The record matrix combines triples of known raw/decoded records and checks each
decoded command, timing, byte range and original bytes.
The example tests run Cargo offline with isolated input and output
paths, and compare file contents and process results.

## Project baseline

Zsh 5.9 supplied the initial reference. The checked-in inputs and expected
outputs define the project’s regression tests. When an intentional behavior
change affects a case, update its expected result and explain the change alongside
the implementation.

The `w01-backslashes.hist` input was written with Zsh 5.9 using the seven commands
from the upstream W01 history-file regression. Its expected commands are byte
literals in `zsh_5_9_baseline.rs`. It runs with ordinary Cargo tests. [Upstream
case](https://github.com/zsh-users/zsh/blob/zsh-5.9/Test/W01history.ztst#L91-L112).

## Fixed history samples

`plain.hist`, `extended.hist` and `mixed.hist` store the same commands in three
formats. They share `commands.expected`, which holds each command's byte length
as a little-endian `u64` followed by its bytes. Tests check command order, timing,
source ranges, and filtered raw records across several reader buffer sizes.

CI runs the default and all-feature tests, formatting, Clippy and documentation checks.
