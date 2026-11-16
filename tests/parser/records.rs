use super::support::{entry, parse_with_capacity, reason};
use zsh_history::{ParseError, ZshParser};

#[test]
fn metafy_pairs_and_literal_legacy_bytes() {
    let mut input = Vec::new();
    let mut expected = Vec::new();
    for b in 1..=255 {
        if b == b'\n' {
            continue;
        }
        input.extend_from_slice(&[b'v', 0x83, b, b'!', b'\n']);
        expected.push(vec![b'v', b ^ 0x20, b'!']);
        input.extend_from_slice(&[b'v', b, 0x84, b'!', b'\n']);
        if b == 0x83 {
            expected.push(vec![b'v', 0xa4, b'!']);
        } else {
            expected.push(vec![b'v', b, 0x84, b'!']);
        }
    }
    for capacity in [1, 7, 8192] {
        let out = parse_with_capacity(&ZshParser::new(), &input, capacity);
        assert_eq!(out.len(), expected.len());
        for (i, (record, command)) in out.iter().zip(&expected).enumerate() {
            assert_eq!(
                &entry(record).command,
                command,
                "record {i}, buffer {capacity}"
            );
        }
    }
}

#[test]
fn legacy_normalization_precedes_header_recognition() {
    // The first pair decodes to ':'. A literal legacy token triggers normalization.
    let input = b"\x83\x1a12:3;cmd\x84\n\x83\x1a12:3;cmd\n\x83\x1a12:3;\x83\x20\xa2\n";
    let out = parse_with_capacity(&ZshParser::new(), input, 1);
    assert_eq!(out.len(), 3);
    assert_eq!(entry(&out[0]).command, b"cmd\x84");
    assert_eq!(entry(&out[0]).timing.timestamp, 12);
    assert_eq!(entry(&out[1]).command, b":12:3;cmd");
    assert!(!entry(&out[1]).timing.from_file);
    assert_eq!(entry(&out[2]).command, b"\0\xa2");
}

#[test]
fn grown_read_buffer_is_reused_for_the_next_record() {
    let first = vec![b'a'; 2048];
    let last = vec![b'b'; 1023];
    let input = [first.as_slice(), b"\n", &last].concat();
    for capacity in [1, 1023, 1024, 8192] {
        let out = parse_with_capacity(&ZshParser::new(), &input, capacity);
        assert_eq!(out.len(), 2);
        assert_eq!(entry(&out[0]).command, first);
        assert_eq!(entry(&out[1]).command, last);
        assert_eq!(entry(&out[1]).source.span, 2049..3072);
    }
}

#[test]
fn continuation_at_a_full_read_segment_preserves_the_next_record() {
    for prefix_len in [1021, 1022, 1023, 2046] {
        let prefix = vec![b'a'; prefix_len];
        let input = [prefix.as_slice(), b"\\\nnext\nlast\n"].concat();
        let expected = [prefix.as_slice(), b"\nnext"].concat();
        for capacity in [1, 19, 8192] {
            let out = parse_with_capacity(&ZshParser::new(), &input, capacity);
            assert_eq!(out.len(), 2);
            assert_eq!(entry(&out[0]).command, expected);
            assert_eq!(entry(&out[1]).command, b"last");
        }
    }
}

#[test]
fn raw_nul_position_inside_final_segment_controls_termination() {
    let prefix = vec![b'a'; 1023];
    for capacity in [1, 7, 8192] {
        for tail in [b"\0x".as_slice(), b"z\0x\nignored\n"] {
            let input = [prefix.as_slice(), tail].concat();
            let out = parse_with_capacity(&ZshParser::new(), &input, capacity);
            assert_eq!(out.len(), 1);
            assert!(matches!(reason(&out[0]), ParseError::CorruptHistory));
        }
        let input = [prefix.as_slice(), b"z\0x"].concat();
        let out = parse_with_capacity(&ZshParser::new(), &input, capacity);
        assert_eq!(out.len(), 1);
        assert_eq!(entry(&out[0]).command, [prefix.as_slice(), b"z"].concat());
        assert_eq!(entry(&out[0]).source.raw, input);
    }
}

#[test]
fn corruption_preserves_consumed_record_and_unfinished_tail_has_no_entry() {
    let p = ZshParser::new();
    let out = parse_with_capacity(&p, b"ok\n1234\\\nA\0B\nafter\n", 1);
    assert_eq!(out.len(), 2);
    assert_eq!(entry(&out[0]).command, b"ok");
    let zsh_history::ParseOutcome::Skipped {
        source,
        reason: ParseError::CorruptHistory,
    } = &out[1]
    else {
        panic!("{:?}", out[1]);
    };
    assert_eq!(source.span, 3..13);
    assert_eq!(source.raw, b"1234\\\nA\0B\n");
    let tail = parse_with_capacity(&p, b"ok\n1234\\\n", 1);
    assert_eq!(tail.len(), 1);
    assert_eq!(entry(&tail[0]).command, b"ok");
}

#[test]
fn accepts_more_than_four_mib_as_one_record() {
    let mut input = vec![b'x'; 4 * 1024 * 1024 + 1];
    input.extend_from_slice(b"\nend\n");
    let out = parse_with_capacity(&ZshParser::new(), &input, 8192);
    assert_eq!(out.len(), 2);
    assert_eq!(entry(&out[0]).command, &input[..input.len() - 5]);
    assert_eq!(entry(&out[1]).command, b"end");
}

#[test]
fn mixed_records_have_exact_boundaries() {
    // Use known raw/decoded pairs as an independent model. Permutations exercise
    // short/long transitions, empty records and duplicates at every position.
    type Case = (&'static [u8], &'static [u8], Option<(i64, i64)>);
    let records: &[Case] = &[
        (b"\n", b"", None),
        (b": 1:0;cmd\n", b"cmd", Some((1, 0))),
        (b":9:4;\n", b"", Some((9, 4))),
        (b"\\:foo\n", b":foo", None),
        (b":10:11;foo\x83\x20\n", b"foo\0", Some((10, 1))),
        (b"one\\\n\\\nend\\  \n", b"one\n\nend\\ ", None),
        (b"\xff\r\n", b"\xff\r", None),
    ];
    for &a in records {
        for &b in records {
            for &c in records {
                let input = [a.0, b.0, c.0].concat();
                let out = parse_with_capacity(&ZshParser::new(), &input, 3);
                assert_eq!(out.len(), 3, "{input:?}");
                for ((raw, command, timing), record) in [a, b, c].into_iter().zip(out) {
                    let e = entry(&record);
                    assert_eq!(e.command, command);
                    let t = e.timing;
                    assert_eq!(t.from_file.then_some((t.timestamp, t.duration)), timing);
                    assert_eq!(e.source.raw, raw);
                }
            }
        }
    }
}
