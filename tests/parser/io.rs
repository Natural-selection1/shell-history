use super::support::{entry, parse, reason};
use std::io::{self, BufRead, Read};
use zsh_history::{ParseError, ParseOutcome, ZshParser};

struct Faulty<'a> {
    data: &'a [u8],
    pos: usize,
    interrupt: bool,
}
impl BufRead for Faulty<'_> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        if self.interrupt {
            self.interrupt = false;
            return Err(io::ErrorKind::Interrupted.into());
        }
        if self.pos == self.data.len() {
            return Err(io::Error::other("injected"));
        }
        Ok(&self.data[self.pos..(self.pos + 2).min(self.data.len())])
    }
    fn consume(&mut self, n: usize) {
        self.pos += n;
    }
}
impl Read for Faulty<'_> {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        let input = self.fill_buf()?;
        let n = input.len().min(b.len());
        b[..n].copy_from_slice(&input[..n]);
        self.consume(n);
        Ok(n)
    }
}

#[test]
fn interruptions_retry_and_io_errors_preserve_consumed_bytes_then_stop() {
    let input = b"ok\npart\\\nmore";
    let p = ZshParser::new();
    let mut iter = p.parse(Faulty {
        data: input,
        pos: 0,
        interrupt: true,
    });
    assert_eq!(entry(&iter.next().unwrap()).command, b"ok");
    let out = iter.next().unwrap();
    match out {
        ParseOutcome::Skipped {
            source,
            reason: ParseError::Io(_),
        } => {
            assert_eq!(source.raw, input[3..]);
            assert_eq!(source.span, 3..input.len() as u64);
        }
        _ => panic!("{out:?}"),
    }
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}

#[test]
fn empty_stream_and_immediate_io_error() {
    let p = &ZshParser::new();
    assert!(parse(p, b"").is_empty());
    let outcomes: Vec<_> = p
        .parse(Faulty {
            data: b"",
            pos: 0,
            interrupt: true,
        })
        .collect();
    assert_eq!(outcomes.len(), 1);
    assert!(matches!(reason(&outcomes[0]), ParseError::Io(_)));
}

#[test]
fn iterator_reads_only_the_requested_records_from_a_borrowed_reader() {
    use std::io::{BufReader, Cursor, Seek};
    let input = b"first\\\nline\nsecond\nthird";
    let mut reader = BufReader::with_capacity(1, Cursor::new(input));
    let parser = ZshParser::new();
    {
        let mut iter = parser.parse(&mut reader);
        assert_eq!(entry(&iter.next().unwrap()).command, b"first\nline");
    }
    assert_eq!(reader.stream_position().unwrap(), 12);
    let rest = parser.parse_vec(&mut reader);
    let commands: Vec<_> = rest.iter().map(|e| e.command.as_slice()).collect();
    assert_eq!(commands, [b"second".as_slice(), b"third"]);
    assert_eq!(rest[0].source.span, 0..7);
}

#[test]
fn interruptions_between_fragments_preserve_multiline_input() {
    use std::io::BufReader;
    struct InterruptedBytes<'a> {
        bytes: &'a [u8],
        interruptions: u8,
    }
    impl Read for InterruptedBytes<'_> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if buf.is_empty() {
                return Ok(0);
            }
            if self.interruptions > 0 {
                self.interruptions -= 1;
                return Err(io::ErrorKind::Interrupted.into());
            }
            if let Some((&b, rest)) = self.bytes.split_first() {
                buf[0] = b;
                self.bytes = rest;
                self.interruptions = 2;
                return Ok(1);
            }
            Ok(0)
        }
    }
    let input = b":9:3;a\\\nb\x83\x20\nc\n";
    let reader = BufReader::new(InterruptedBytes {
        bytes: input,
        interruptions: 2,
    });
    let parser = ZshParser::new();
    let mut iter = parser.parse(reader);
    let first = iter.next().unwrap();
    assert_eq!(entry(&first).command, b"a\nb\0");
    assert_eq!(entry(&first).source.raw, &input[..input.len() - 2]);
    assert_eq!(entry(&iter.next().unwrap()).command, b"c");
    assert!(iter.next().is_none());
    assert!(iter.next().is_none());
}
