# Reading Zsh history

The initial compatibility baseline is the history list produced by one `fc -R` into an empty history
context in 64-bit Zsh 5.9, with default options and enough `HISTSIZE` to hold the
input. The iterator yields records in file order, including duplicates and empty
commands. History eviction, interactive editing, incremental imports into a
running session and deduplication by shell options operate on session state.

## Commands and source bytes

The parser joins continued physical lines, removes Zsh's trailing backslash
padding and decodes metafy pairs. A trailing metafy marker represents a literal
marker. Older files with unescaped internal token bytes undergo Zsh's encoding
normalization before header recognition. The decoded command may contain any
byte, including an encoded NUL. Its original record remains available in
`source.raw` with a half-open byte range.

A final line can end at EOF. Zsh drops a pending record when a continuation or a
full input-buffer segment requires another read and that read returns no bytes.
The initial Zsh buffer holds 1024 bytes including its terminator and can grow
across records. An external Rust reader's buffer size does not affect this rule.

A raw NUL in a newline-terminated input segment ends the import as corrupt.
The parser reports `CorruptHistory` once and retains the preceding entries.
In a short final segment ending at EOF, Zsh accepts a nonempty prefix before
NUL. A NUL at the very start of the segment is corruption. An ignored EOF tail
has no corresponding history entry.

## Time fields

A leading unescaped colon starts a header. Zsh scans for the next colon and
semicolon even if a numeric field is empty or ends in other characters. Missing
delimiters can produce an empty command. Numeric prefixes follow 64-bit
`zstrtol`, including signs, binary, octal, hex and overflow truncation.

Every parsed entry has a `Timing` value. A missing or zero start time takes the
time at the beginning of the import and has zero duration. `Timing::from_file`
indicates a nonzero start time from the file. For a nonzero start, a second
field smaller than the start is a duration. Otherwise it is an absolute finish
time. The elapsed value follows Zsh's history listing, including zero for a
zero effective finish.

The clean_zsh example uses file order when any entry has a substituted start
time. This keeps a newly imported plain entry from outranking an older duplicate
solely because the file was opened today.

The implementation follows the upstream [history reader](https://github.com/zsh-users/zsh/blob/zsh-5.9/Src/hist.c),
[numeric and metafy conversions](https://github.com/zsh-users/zsh/blob/zsh-5.9/Src/utils.c)
and [elapsed-time display](https://github.com/zsh-users/zsh/blob/zsh-5.9/Src/builtin.c).

## Operational considerations

The parser retains each logical record in full. Its working memory grows with
the largest record. The cleaner also retains entries for deduplication.
Rust I/O errors are reported to the
caller. Zsh's numeric truncation warnings are not emitted to stderr by the
library.

Filtering copies stored source bytes, including the original time header. Write
filtered output to a separate file. Zsh sessions can append history while an
external program is reading, and replacing a live history file requires
coordination with those sessions.
