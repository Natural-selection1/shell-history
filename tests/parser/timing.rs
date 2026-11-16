use super::support::{entry, parse};
use std::time::{SystemTime, UNIX_EPOCH};
use zsh_history::{Timing, ZshParser};

#[test]
fn signed_numeric_prefixes_and_elapsed_time() {
    let cases: &[(&[u8], i64, i64)] = &[
        (b":10:3;cmd", 10, 3),
        (b":10:10;cmd", 10, 0),
        (b":10:13;cmd", 10, 3),
        (b":10:-4;cmd", 10, -4),
        (b":10:-10;cmd", 10, 0),
        (b":-10:-14;cmd", -10, -14),
        (b":-10:-3;cmd", -10, 7),
        (b":-10:0;cmd", -10, 0),
        (b":\t+0X2a ignored:0B101junk;cmd", 42, 5),
        (b":+019:099;cmd", 1, 0),
        (b":15_0:2_0;cmd", 15, 2),
        (b":\\\n+45:0x10;cmd", 45, 16),
        (b": -9223372036854775808:-2;cmd", i64::MIN, i64::MAX - 1),
        (b": 9223372036854775808:0;cmd", 922337203685477580, 0),
        (b": +000000000000000000000000000001:0;cmd", 1, 0),
        (b":9223372036854775807:2;cmd", i64::MAX, 2),
        (
            b":-9223372036854775809:-2;cmd",
            -922337203685477580,
            922337203685477578,
        ),
        (b":18446744073709551616:0;cmd", 1844674407370955161, 0),
    ];
    for &(raw, timestamp, duration) in cases {
        let out = parse(&ZshParser::new(), raw);
        assert_eq!(out.len(), 1, "{raw:?}");
        let e = entry(&out[0]);
        assert_eq!(e.command, b"cmd", "{raw:?}");
        assert_eq!(
            e.timing,
            Timing {
                from_file: true,
                timestamp,
                duration
            },
            "{raw:?}"
        );
    }
}

#[test]
fn absent_zero_and_invalid_starts_share_import_time() {
    let now = || {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64
    };
    let before = now();
    let out = parse(&ZshParser::new(), b"plain\n:0:99;zero\n::;missing\n:-0:4;negative zero\n:0x:1;hex\n:08:3;octal\n:\r3:8;cr\n:no:99;bad\n: broken\\\nheader\n");
    let after = now();
    let expected: &[&[u8]] = &[
        b"plain",
        b"zero",
        b"missing",
        b"negative zero",
        b"hex",
        b"octal",
        b"cr",
        b"bad",
        b"",
    ];
    assert_eq!(out.len(), expected.len());
    let stamp = entry(&out[0]).timing.timestamp;
    assert!((before..=after).contains(&stamp));
    for (record, command) in out.iter().zip(expected) {
        let e = entry(record);
        assert_eq!(&e.command, command);
        assert_eq!(
            e.timing,
            Timing {
                timestamp: stamp,
                duration: 0,
                from_file: false
            }
        );
    }
}

#[test]
fn missing_delimiters_and_literal_delimiters_in_command() {
    let out = parse(
        &ZshParser::new(),
        b":12\n:12;not a command\n:12:3\n:12::;:a;b:c\n\\:12:3;literal\n",
    );
    let expected: &[(&[u8], i64)] = &[(b"", 0), (b"", 0), (b"", 3), (b":a;b:c", 0)];
    assert_eq!(out.len(), 5);
    for (record, &(command, duration)) in out.iter().zip(expected) {
        assert_eq!(entry(record).command, command);
        assert_eq!(
            entry(record).timing,
            Timing {
                timestamp: 12,
                duration,
                from_file: true
            }
        );
    }
    assert_eq!(entry(&out[4]).command, b":12:3;literal");
    assert!(!entry(&out[4]).timing.from_file);
}
