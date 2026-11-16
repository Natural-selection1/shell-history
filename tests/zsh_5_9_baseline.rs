//! Port of “Lines ending in backslash saved and restored to history” from
//! https://github.com/zsh-users/zsh/blob/zsh-5.9/Test/W01history.ztst .

use std::io::BufReader;
use zsh_history::ZshParser;

#[test]
fn lines_ending_in_backslash_saved_and_restored() {
    let history = include_bytes!("fixtures/w01-backslashes.hist");
    let expected: &[&[u8]] = &[
        br"one\",
        br"two\\",
        br"three\\\",
        br"four\\\\",
        br"five\\\\\",
        b"while false\ndo\ntrue\\\n && break\ndone",
        b"echo one\\\ntwo",
    ];

    for capacity in [1, 8192] {
        let reader = BufReader::with_capacity(capacity, &history[..]);
        let parser = ZshParser::new();
        let mut records = parser.parse(reader);
        for (index, &command) in expected.iter().enumerate() {
            let record = records.next().expect("missing history entry");
            let entry = record.entry().expect("skipped history entry");
            assert_eq!(entry.command, command, "record {index}, buffer {capacity}");
        }
        assert!(records.next().is_none(), "extra entry, buffer {capacity}");
    }
}
