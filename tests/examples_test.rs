//! Exercise the examples through their public command lines with private test inputs.
use std::{
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

struct Sandbox(PathBuf);

impl Sandbox {
    fn new(input: &[u8]) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = loop {
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            let dir =
                std::env::temp_dir().join(format!("zsh-history-test-{}-{id}", std::process::id()));
            match fs::create_dir(&dir) {
                Ok(()) => break dir,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => panic!("could not create test directory: {e}"),
            }
        };
        fs::write(dir.join("input"), input).unwrap();
        Self(dir)
    }

    fn run(&self, example: &str, args: &[impl AsRef<OsStr>]) -> Output {
        // Cargo exposes CARGO_BIN_EXE for binaries only. Running the example
        // through Cargo also works when this integration test is run by itself.
        let mut cmd = Command::new(env!("CARGO"));
        cmd.current_dir(&self.0)
            .args(["run", "--offline", "--locked", "--quiet", "--manifest-path"])
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"));
        if cfg!(feature = "memchr") {
            cmd.arg("--features").arg("memchr");
        }
        cmd.args(["--example", example, "--"])
            .args(args)
            .output()
            .unwrap()
    }

    fn read(&self, name: &str) -> Vec<u8> {
        fs::read(self.0.join(name)).unwrap()
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn success(result: &Output) {
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

#[test]
fn clean_keeps_latest_then_later_tie_and_copies_selected_bytes_in_file_order() {
    let input = b":40:1;git status\n:10:0;code\n:9:0;git status\n:41:0;multi\\\nline\n:41:5;multi\\\nline\n:12:0;y\n:11:0;code .\n:6:0; code\n:13:0;Y\n:17:0;code ";
    let dir = Sandbox::new(input);
    let result = dir.run("clean_zsh", &["input", "output", "y", "code"]);
    success(&result);
    assert!(result.stdout.is_empty());
    assert_eq!(dir.read("input"), input);
    assert_eq!(
        dir.read("output"),
        b":40:1;git status\n:41:5;multi\\\nline\n:11:0;code .\n:6:0; code\n:13:0;Y\n:17:0;code "
    );
    assert!(
        String::from_utf8_lossy(&result.stderr)
            .contains("parsed=10, excluded=2, duplicates=2, kept=6, order=timestamp")
    );
}

#[test]
fn clean_uses_file_order_if_any_start_was_substituted() {
    for marker in [b"plain\n".as_slice(), b":0:12;zero\n"] {
        let input = [b":90:0;git\n".as_slice(), marker, b":5:0;git\n"].concat();
        let dir = Sandbox::new(&input);
        let result = dir.run("clean_zsh", &["input", "output"]);
        success(&result);
        assert_eq!(dir.read("output"), [marker, b":5:0;git\n"].concat());
        assert!(String::from_utf8_lossy(&result.stderr).contains("order=file"));
    }
}

#[test]
fn clean_deduplicates_decoded_bytes_and_preserves_the_winning_encoding() {
    let dir = Sandbox::new(b"\\:literal\n:12:3;:literal\nx\xff\nx\xfe\nx\xff\n\n\n");
    success(&dir.run("clean_zsh", &["input", "output"]));
    assert_eq!(dir.read("output"), b":12:3;:literal\nx\xfe\nx\xff\n\n");
    success(&dir.run("clean_zsh", &["output", "again"]));
    assert_eq!(dir.read("again"), dir.read("output"));
}

#[test]
fn clean_handles_empty_input_and_complete_exclusion() {
    for raw in [b"".as_slice(), b"y\ny\ncode\n"] {
        let dir = Sandbox::new(raw);
        success(&dir.run("clean_zsh", &["input", "output", "y", "code"]));
        assert!(dir.read("output").is_empty());
    }
}

#[test]
fn clean_refuses_to_overwrite_and_creates_no_output_on_read_failure() {
    let dir = Sandbox::new(b":1:0;ok\n");
    fs::write(dir.0.join("existing"), b"keep me").unwrap();
    for target in ["existing", "input"] {
        assert!(!dir.run("clean_zsh", &["input", target]).status.success());
    }
    assert_eq!(dir.read("existing"), b"keep me");
    assert_eq!(dir.read("input"), b":1:0;ok\n");
    fs::write(dir.0.join("input"), b"before\nbad\0record\nafter\n").unwrap();
    let bad = dir.run("clean_zsh", &["input", "output"]);
    assert!(!bad.status.success());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("7..18: corrupt history file"));
    assert!(
        !dir.run("clean_zsh", &["missing", "output"])
            .status
            .success()
    );
    assert!(!dir.0.join("output").exists());
}

#[cfg(unix)]
#[test]
fn clean_accepts_non_utf8_exclusions_and_creates_private_output() {
    use std::os::unix::{ffi::OsStrExt, fs::PermissionsExt};
    let dir = Sandbox::new(b"x\xff\nx\xfe\n");
    let args = [
        OsStr::new("input"),
        OsStr::new("output"),
        OsStr::from_bytes(b"x\xff"),
    ];
    success(&dir.run("clean_zsh", &args));
    assert_eq!(dir.read("output"), b"x\xfe\n");
    assert_eq!(
        fs::metadata(dir.0.join("output"))
            .unwrap()
            .permissions()
            .mode()
            & 0o077,
        0
    );
}

#[test]
fn filter_matches_decoded_prefix_and_reports_corruption_after_valid_output() {
    let raw = b"\\:first\n:10:0;:multi\\\nline\nother\n";
    let dir = Sandbox::new(raw);
    let result = dir.run("filter_zsh", &["input", ":"]);
    success(&result);
    assert_eq!(result.stdout, b"\\:first\n:10:0;:multi\\\nline\n");
    fs::write(
        dir.0.join("input"),
        [raw.as_slice(), b"a\0b\nunread\n"].concat(),
    )
    .unwrap();
    let bad = dir.run("filter_zsh", &["input", ""]);
    assert!(!bad.status.success());
    assert_eq!(bad.stdout, raw);
    assert!(String::from_utf8_lossy(&bad.stderr).contains("corrupt history file"));
}

#[test]
fn dump_renders_timestamps_multiline_and_non_utf8_and_reports_skips() {
    let dir = Sandbox::new(b":12:1;line\\\n\xff\ncorrupt\0x\nunread\n");
    let result = dir.run("dump_zsh", &["input"]);
    success(&result);
    assert_eq!(result.stdout, "[12] line⏎�\n".as_bytes());
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("corrupt history file"));
    assert!(stderr.contains("1 entries, 1 skipped"));
}

#[test]
fn clean_can_exclude_an_empty_or_multiline_command_as_one_argument() {
    let dir = Sandbox::new(b"\none\\\ntwo\none\ntwo\n\n");
    success(&dir.run("clean_zsh", &["input", "output", "", "one\ntwo"]));
    assert_eq!(dir.read("output"), b"one\ntwo\n");
}

#[test]
fn missing_cli_arguments_report_the_required_input() {
    let dir = Sandbox::new(b"data\n");
    for (example, args, message) in [
        ("clean_zsh", &[][..], "<NEW_OUTPUT>"),
        ("clean_zsh", &["input"][..], "<NEW_OUTPUT>"),
        ("filter_zsh", &[][..], "<INPUT>"),
        ("filter_zsh", &["input"][..], "<PREFIX>"),
    ] {
        let result = dir.run(example, args);
        assert_eq!(result.status.code(), Some(2));
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(
            stderr
                .lines()
                .take_while(|line| !line.is_empty())
                .any(|line| line.trim() == message),
            "{stderr}"
        );
    }
}
