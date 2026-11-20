mod builtin;
mod lexer;
mod parser;
mod pty;
mod screen;
mod vt;
mod window;

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdout, ExitStatus, Stdio};

enum PrevInput {
    Pipe(ChildStdout),
    Buffer(Vec<u8>),
}

pub struct Shell {
    pub cwd: PathBuf,
    pub prev: Option<PathBuf>,
}

#[derive(Clone)]
pub struct ShellSnapshot {
    pub cwd: PathBuf,
    pub prev: Option<PathBuf>,
}

impl Shell {
    fn new() -> Self {
        Self {
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            prev: None,
        }
    }

    fn print_prompt(&self) {
        print!("{}# ", self.cwd.display());
        io::stdout().flush().unwrap();
    }

    pub fn snapshot(&self) -> ShellSnapshot {
        ShellSnapshot {
            cwd: self.cwd.clone(),
            prev: self.prev.clone(),
        }
    }
}

fn run_external(cmd: &parser::Command, cwd: &Path) -> io::Result<ExitStatus> {
    run_external_cmd(cmd, cwd, Stdio::inherit(), Stdio::inherit())?.wait()
}

fn run_external_cmd(
    cmd: &parser::Command,
    cwd: &Path,
    stdin: Stdio,
    stdout: Stdio,
) -> io::Result<Child> {
    std::process::Command::new(&cmd.program)
        .args(&cmd.args)
        .current_dir(cwd)
        .env("NO_COLOR", "1")
        .stdin(stdin)
        .stdout(stdout)
        .spawn()
}
fn run_pipeline(snapshot: &ShellSnapshot, pipeline: &parser::Pipeline) -> io::Result<()> {
    let n = pipeline.commands.len();
    let mut children: Vec<Option<Child>> = Vec::new();
    let mut prev: Option<PrevInput> = None;

    for (i, cmd) in pipeline.commands.iter().enumerate() {
        let is_last = i == n - 1;
        let mut pending: Option<Vec<u8>> = None;
        // 输入端
        let stdin = match prev.take() {
            Some(PrevInput::Pipe(out)) => Stdio::from(out),
            Some(PrevInput::Buffer(v)) => {
                pending = Some(v);
                Stdio::piped()
            }
            None => Stdio::inherit(),
        };

        // 是外部程序吗？
        let mut probe = Vec::new();
        if builtin::is_builtin(&cmd.program) {
            // 走内置：写进 Vec，把 Vec 当下一段的 stdin
            let outcome = builtin::run_builtin(snapshot, cmd, &mut probe);
            match outcome {
                Some(builtin::CmdOutcome::Exit) => break,
                // todo 后续直接并到None
                Some(builtin::CmdOutcome::Done) => {}
                // 管道里每段独立，cd 不影响父 shell
                Some(builtin::CmdOutcome::ChangeDir(_)) => {}
                None => {}
            }
            if is_last {
                let _ = io::stdout().write_all(&probe);
            } else {
                prev = Some(PrevInput::Buffer(probe.clone()));
            }
        } else {
            // 走外部：run_external_cmd
            let stdout = if is_last {
                Stdio::inherit()
            } else {
                Stdio::piped()
            };
            let mut child = run_external_cmd(cmd, &snapshot.cwd, stdin, stdout)?;
            if let Some(v) = pending {
                let mut sin = child.stdin.take().unwrap();
                sin.write_all(&v)?; // 注意：sin 在这里被 drop，管道关闭，下游读到 EOF
            }
            prev = child.stdout.take().map(PrevInput::Pipe);
            children.push(Some(child));
        }
    }
    for c in children.iter_mut().flatten() {
        c.wait()?;
    }
    Ok(())
}

fn main() {
    if std::env::args().nth(1).as_deref() == Some("gui") {
        window::run();
        return;
    }
    if std::env::args().nth(1).as_deref() == Some("pty") {
        pty::run();
        return;
    }

    let mut shell = Shell::new();
    loop {
        shell.print_prompt();
        let mut line = String::new();
        match io::stdin().read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {
                let tokens: Vec<lexer::Token> = lexer::lex(&line);
                if tokens.is_empty() {
                    continue;
                }
                let pipeline = match parser::parse(&tokens) {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("解析失败: {}", e.message);
                        continue;
                    }
                };
                let cmd = match pipeline.commands.as_slice() {
                    [cmd] => cmd,
                    [] => {
                        eprintln!("没有命令");
                        continue;
                    }
                    _ => {
                        if let Err(e) = run_pipeline(&shell.snapshot(), &pipeline) {
                            eprintln!("管道执行失败: {e}");
                        }
                        continue;
                    }
                };
                let snapshot = shell.snapshot();
                match builtin::run_builtin(&snapshot, cmd, &mut io::stdout()) {
                    Some(builtin::CmdOutcome::Exit) => break,
                    Some(builtin::CmdOutcome::Done) => {}
                    Some(builtin::CmdOutcome::ChangeDir(new_cwd)) => {
                        shell.prev = Some(std::mem::replace(&mut shell.cwd, new_cwd));
                    }
                    None => match run_external(cmd, &shell.cwd) {
                        Ok(status) => println!("{status}"),
                        Err(e) => eprintln!("{e}"),
                    },
                }
            }
            Err(e) => {
                eprintln!("读输入失败: {e}");
                break;
            }
        }
    }
}
