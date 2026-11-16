//! Offline read-back samples recorded with Zsh 5.9.
use std::io::BufReader;
use zsh_history::ZshParser;

fn check(input: &[u8], timestamp_at: fn(usize) -> bool) {
    let parser = ZshParser::new();
    let mut expected = Vec::new();
    let mut rest = include_bytes!("fixtures/commands.expected").as_slice();
    while !rest.is_empty() {
        assert!(rest.len() >= 8, "incomplete oracle length");
        let len = usize::try_from(u64::from_le_bytes(rest[..8].try_into().unwrap()))
            .expect("oracle length must fit usize");
        rest = &rest[8..];
        assert!(len <= rest.len(), "incomplete oracle command");
        expected.push(&rest[..len]);
        rest = &rest[len..];
    }
    for capacity in [1, 2, 3, 7, 8192] {
        let results: Vec<_> = parser
            .parse(BufReader::with_capacity(capacity, input))
            .collect();
        let entries: Vec<_> = results
            .iter()
            .map(|r| r.entry().unwrap_or_else(|| panic!("{r:?}")))
            .collect();
        assert_eq!(
            entries
                .iter()
                .map(|e| e.command.as_slice())
                .collect::<Vec<_>>(),
            expected,
            "buffer {capacity}"
        );
        let mut restored = Vec::new();
        let mut filtered = Vec::new();
        for (i, entry) in entries.iter().enumerate() {
            let src = &entry.source;
            assert_eq!(src.span.start, restored.len() as u64);
            assert_eq!(
                src.raw,
                input[src.span.start as usize..src.span.end as usize]
            );
            restored.extend_from_slice(&src.raw);
            if i % 2 == 0 {
                filtered.extend_from_slice(&src.raw);
            }
            assert_eq!(entry.timing.from_file, timestamp_at(i), "entry {i}");
            assert_eq!(entry.timing.duration, 0);
        }
        assert_eq!(restored, input);
        let selected: Vec<_> = parser
            .parse(&filtered[..])
            .map(|r| {
                let e = r.entry().expect("filtered source must still parse");
                (e.command.clone(), e.timing.from_file.then_some(e.timing))
            })
            .collect();
        let wanted: Vec<_> = entries
            .iter()
            .enumerate()
            .filter(|(i, _)| i % 2 == 0)
            .map(|(_, e)| (e.command.clone(), e.timing.from_file.then_some(e.timing)))
            .collect();
        assert_eq!(selected, wanted, "buffer {capacity}");
    }
}

#[test]
fn zsh_plain_matches_shell_readback() {
    check(include_bytes!("fixtures/plain.hist"), |_| false);
}

#[test]
fn zsh_extended_matches_shell_readback() {
    check(include_bytes!("fixtures/extended.hist"), |_| true);
}

#[test]
fn zsh_mixed_matches_shell_readback() {
    check(include_bytes!("fixtures/mixed.hist"), |i| i % 2 == 1);
}
