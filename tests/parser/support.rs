use std::io::BufReader;
use zsh_history::{HistoryEntry, ParseError, ParseOutcome, ZshParser};

pub fn parse(p: &ZshParser, input: &[u8]) -> Vec<ParseOutcome> {
    parse_with_capacity(p, input, 2)
}

pub fn parse_with_capacity(p: &ZshParser, input: &[u8], capacity: usize) -> Vec<ParseOutcome> {
    let outcomes: Vec<_> = p.parse(BufReader::with_capacity(capacity, input)).collect();
    let mut offset = 0;
    for out in &outcomes {
        let src = match out {
            ParseOutcome::Entry(e) => &e.source,
            ParseOutcome::Skipped { source, .. } => source,
        };
        assert_eq!(src.span.start, offset);
        assert!(src.span.end >= src.span.start);
        assert!(src.span.end <= input.len() as u64);
        assert_eq!(src.raw, input[offset as usize..src.span.end as usize]);
        offset = src.span.end;
    }
    if offset < input.len() as u64 {
        let tail = &input[offset as usize..];
        let corrupt = matches!(
            outcomes.last(),
            Some(ParseOutcome::Skipped {
                reason: ParseError::CorruptHistory,
                ..
            })
        );
        assert!(
            corrupt || tail.ends_with(b"\\\n") || !tail.ends_with(b"\n"),
            "unexplained tail: {tail:?}"
        );
    }
    outcomes
}
pub fn entry(out: &ParseOutcome) -> &HistoryEntry {
    out.entry().unwrap_or_else(|| panic!("{out:?}"))
}
pub fn reason(out: &ParseOutcome) -> &ParseError {
    match out {
        ParseOutcome::Skipped { reason, .. } => reason,
        _ => panic!("{out:?}"),
    }
}
