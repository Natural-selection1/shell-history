use super::support::{entry, parse, reason};
use zsh_history::{ParseError, ZshParser};

#[test]
fn zsh_escaping() {
    let cases: &[(&[u8], &[u8])] = &[
        (b"\\: colon\n", b": colon"),
        (b"\\: 12:4;literal\n", b": 12:4;literal"),
        (b": 12:4;\\: literal\n", b"\\: literal"),
        (b"two\\\\ \n", b"two\\\\"),
        (b"space\\  \n", b"space\\ "),
        (b"spaces\\    \n", b"spaces\\   "),
        (b"literal\\\\\nnewline\n", b"literal\\\nnewline"),
        (b"blank\\\n\\\nline\n", b"blank\n\nline"),
        (b"trailing\\\n\n", b"trailing\n"),
        (b"\x83\xa3\x83\x20\xff\n", b"\x83\0\xff"),
        (b"meta\x83\n", b"meta\x83"),
        (b"last\\", b"last\\"),
        (b"last\\ ", b"last\\ "),
        (b"carriage\r\n", b"carriage\r"),
        (b"carriage\\\r\n", b"carriage\\\r"),
        (b"spaces   \n", b"spaces   "),
    ];
    for &(input, expected) in cases {
        let out = parse(&ZshParser::new(), input);
        assert_eq!(out.len(), 1, "{input:?}");
        assert_eq!(entry(&out[0]).command, expected, "{input:?}");
    }
}

#[test]
fn eof_and_corrupt_input_match_fc_read_boundaries() {
    let p = ZshParser::new();
    for len in [1022, 1023, 1024, 2047, 2048] {
        let data = vec![b'x'; len];
        let out = parse(&p, &data);
        assert_eq!(
            out.len(),
            usize::from(![1023, 2047].contains(&len)),
            "{len}"
        );
    }
    let out = parse(&p, b"first\nunfinished\\\n");
    assert_eq!(out.len(), 1);
    assert_eq!(entry(&out[0]).command, b"first");
    let out = parse(&p, b"first\nA\0B\nignored\n");
    assert_eq!(out.len(), 2);
    assert!(matches!(reason(&out[1]), ParseError::CorruptHistory));
    assert_eq!(entry(&parse(&p, b"prefix\0tail")[0]).command, b"prefix");
}
