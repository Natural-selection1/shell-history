use super::support::{entry, parse, parse_with_capacity};
use zsh_history::{ParseOutcome, ZshParser};

#[test]
fn command_bytes_and_display_are_separate() {
    let p = &ZshParser::new();
    let out = parse(p, b"echo \xff\n");
    let e = entry(&out[0]);
    assert_eq!(e.command, b"echo \xff");
    assert!(e.command_str().is_err());
    assert_eq!(e.command_lossy(), "echo �");
}

#[test]
fn arbitrary_inputs_preserve_source_and_ignore_buffer_boundaries() {
    // Deterministic byte corpus, weighted toward format punctuation; checks
    // externally observable coverage rather than a particular parser algorithm.
    let alphabet = b"#0123456789:; +-\\\n\r\0\x83\xffabcdef";
    let mut seed = 0xdeadbeefu64;
    for case in 0..1024 {
        let mut input = Vec::new();
        for _ in 0..case % 257 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            input.push(alphabet[(seed >> 32) as usize % alphabet.len()]);
        }
        let parser = &ZshParser::new();
        let expected = parse_with_capacity(parser, &input, 1);
        for capacity in [2, 7, 8192] {
            let actual = parse_with_capacity(parser, &input, capacity);
            let context = format!("case {case}, buffer {capacity}");
            assert_eq!(actual.len(), expected.len(), "{context}");
            for (actual, expected) in actual.iter().zip(&expected) {
                match (actual, expected) {
                    (ParseOutcome::Entry(a), ParseOutcome::Entry(e)) => {
                        assert_eq!(a.command, e.command, "{context}");
                        assert_eq!(a.source, e.source, "{context}");
                        assert_eq!(a.timing.from_file, e.timing.from_file);
                        if a.timing.from_file {
                            assert_eq!(a.timing, e.timing, "{context}");
                        }
                    }
                    (
                        ParseOutcome::Skipped {
                            source: a,
                            reason: ar,
                        },
                        ParseOutcome::Skipped {
                            source: e,
                            reason: er,
                        },
                    ) => {
                        assert_eq!(a, e, "{context}");
                        assert_eq!(
                            std::mem::discriminant(ar),
                            std::mem::discriminant(er),
                            "{context}"
                        );
                        assert_eq!(ar.to_string(), er.to_string(), "{context}");
                    }
                    _ => panic!("{context}: {actual:?} != {expected:?}"),
                }
            }
        }
    }
}

#[test]
fn editing_a_decoded_command_preserves_its_source() {
    let raw = b":30:0;line\\\nnext\n";
    let mut entries = ZshParser::new().parse_vec(&raw[..]);
    assert_eq!(entries.len(), 1);
    let mut parsed = entries.pop().unwrap();
    assert_eq!(parsed.command_str().unwrap(), "line\nnext");
    parsed.command.clear();
    let source = parsed.source;
    assert_eq!(source.raw, raw);
    assert_eq!(source.span, 0..raw.len() as u64);
}

#[test]
fn convenience_collection_preserves_entries_before_corruption() {
    let input = &b"ok\nlong command\ny\ny\na\0b\nunread\n"[..];
    let p = ZshParser::new();
    let out = parse(&p, input);
    assert_eq!(out.len(), 5);
    assert!(out[4].entry().is_none());
    let entries = p.parse_vec(input);
    let commands: Vec<_> = entries.iter().map(|e| e.command.as_slice()).collect();
    assert_eq!(commands, [b"ok".as_slice(), b"long command", b"y", b"y"]);
    let spans: Vec<_> = entries.iter().map(|e| e.source.span.clone()).collect();
    assert_eq!(spans, [0..3, 3..16, 16..18, 18..20]);
}

#[test]
fn public_unmetafy_roundtrips_every_byte_and_keeps_a_dangling_marker() {
    use zsh_history::zsh::unmetafy;
    assert_eq!(unmetafy(b"").as_ref(), b"");
    assert_eq!(unmetafy(b"plain\xff").as_ref(), b"plain\xff");
    let expected: Vec<u8> = (0..=255).collect();
    let encoded: Vec<u8> = expected.iter().flat_map(|b| [0x83, b ^ 0x20]).collect();
    assert_eq!(unmetafy(&encoded).as_ref(), expected);
    assert_eq!(unmetafy(b"\x83\x20\x83").as_ref(), b"\0\x83");
}
