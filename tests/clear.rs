use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn clear_emits_escape_sequence_and_continues() {
    let cwd = std::env::current_dir().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_kria-terminal"))
        .current_dir(&cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"clear\n").unwrap();

    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(
        stdout.ends_with(&format!("\x1b[2J\x1b[H{}# ", cwd.display())),
        "{stdout:?}"
    );
}
