use crate::ShellSnapshot;
use crate::parser::Command;
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;

#[derive(Debug, PartialEq)]
pub enum BuiltinCommand {
    Exit,
    Pwd,
    Cd,
    Clear,
    Ls,
    Mkdir,
}

pub enum CmdOutcome {
    Done,
    ChangeDir(PathBuf),
    Exit,
}

impl FromStr for BuiltinCommand {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "exit" => Ok(BuiltinCommand::Exit),
            "pwd" => Ok(BuiltinCommand::Pwd),
            "cd" => Ok(BuiltinCommand::Cd),
            "clear" => Ok(BuiltinCommand::Clear),
            "ls" => Ok(BuiltinCommand::Ls),
            "mkdir" => Ok(BuiltinCommand::Mkdir),
            _ => Err(()),
        }
    }
}

pub fn is_builtin(name: &str) -> bool {
    BuiltinCommand::from_str(name).is_ok()
}

pub fn run_builtin(
    shell: &ShellSnapshot,
    cmd: &Command,
    stdout: &mut dyn Write,
) -> Option<CmdOutcome> {
    let builtin = BuiltinCommand::from_str(&cmd.program).ok()?;

    Some(match builtin {
        BuiltinCommand::Exit => builtins::exit::run(stdout),
        BuiltinCommand::Pwd => builtins::pwd::run(shell, stdout),
        BuiltinCommand::Cd => builtins::cd::run(shell, cmd, stdout),
        BuiltinCommand::Clear => builtins::clear::run(stdout),
        BuiltinCommand::Ls => builtins::ls::run(shell, cmd, stdout),
        BuiltinCommand::Mkdir => builtins::mkdir::run(shell, cmd, stdout),
    })
}

mod builtins {
    pub mod exit {
        use super::super::*;

        pub fn run(stdout: &mut dyn Write) -> CmdOutcome {
            let _ = writeln!(stdout, "bye");
            CmdOutcome::Exit
        }
    }

    pub mod pwd {
        use super::super::*;

        pub fn run(shell: &ShellSnapshot, stdout: &mut dyn Write) -> CmdOutcome {
            let _ = writeln!(stdout, "{}", shell.cwd.display());
            CmdOutcome::Done
        }
    }

    pub mod clear {
        use super::super::*;

        pub fn run(stdout: &mut dyn Write) -> CmdOutcome {
            let _ = stdout.write_all(b"\x1b[2J\x1b[H");
            let _ = stdout.flush();
            CmdOutcome::Done
        }
    }

    pub mod ls {
        use super::super::*;

        pub fn run(shell: &ShellSnapshot, _cmd: &Command, stdout: &mut dyn Write) -> CmdOutcome {
            match fs::read_dir(&shell.cwd) {
                Ok(entries) => {
                    let mut names: Vec<String> = Vec::new();
                    for entry in entries {
                        match entry {
                            Ok(entry) => {
                                names.push(entry.file_name().to_string_lossy().into_owned())
                            }
                            Err(e) => eprintln!("读取条目失败：{}", e),
                        }
                    }
                    names.sort();
                    for name in names {
                        let _ = writeln!(stdout, "{name}");
                    }
                }
                Err(e) => eprintln!("打开失败：{}", e),
            }
            CmdOutcome::Done
        }
    }

    pub mod cd {
        use super::super::*;

        pub fn run(shell: &ShellSnapshot, cmd: &Command, stdout: &mut dyn Write) -> CmdOutcome {
            match cmd.args.first().map(String::as_str) {
                Some("-") => {
                    if let Some(prev) = shell.prev.as_ref() {
                        let new_cwd = resolve_cd(&shell.cwd, prev);
                        if let Some(new_cwd) = validate_dir(new_cwd) {
                            return CmdOutcome::ChangeDir(new_cwd);
                        }
                    }
                    CmdOutcome::Done
                }
                Some(arg) => {
                    let new_cwd = resolve_cd(&shell.cwd, Path::new(arg));
                    if let Some(new_cwd) = validate_dir(new_cwd) {
                        CmdOutcome::ChangeDir(new_cwd)
                    } else {
                        CmdOutcome::Done
                    }
                }
                None => {
                    if cfg!(windows) {
                        let _ = writeln!(stdout, "{}", shell.cwd.display());
                        CmdOutcome::Done
                    } else if let Some(home) = home_dir() {
                        let new_cwd = resolve_cd(&shell.cwd, &home);
                        if let Some(new_cwd) = validate_dir(new_cwd) {
                            CmdOutcome::ChangeDir(new_cwd)
                        } else {
                            CmdOutcome::Done
                        }
                    } else {
                        eprintln!("cd: HOME not set");
                        CmdOutcome::Done
                    }
                }
            }
        }
    }

    pub mod mkdir {
        use super::super::*;

        pub fn run(shell: &ShellSnapshot, cmd: &Command, _stdout: &mut dyn Write) -> CmdOutcome {
            let mut parents = false;
            let mut enter = false;
            let mut paths: Vec<&str> = Vec::new();
            let mut only_paths = false;

            for arg in &cmd.args {
                if only_paths {
                    paths.push(arg.as_str());
                    continue;
                }
                match arg.as_str() {
                    "--" => only_paths = true,
                    "-p" => parents = true,
                    "-e" => enter = true,
                    a if a.len() > 1 && a.starts_with('-') => {
                        for ch in a.chars().skip(1) {
                            match ch {
                                'p' => parents = true,
                                'e' => enter = true,
                                _ => {
                                    eprintln!("mkdir: unknown option -- '{ch}'");
                                    return CmdOutcome::Done;
                                }
                            }
                        }
                    }
                    _ => paths.push(arg.as_str()),
                }
            }

            if paths.is_empty() {
                eprintln!("mkdir: missing operand");
                return CmdOutcome::Done;
            }
            if enter && paths.len() > 1 {
                eprintln!("mkdir: -e requires exactly one path");
                return CmdOutcome::Done;
            }

            let mut failed = false;
            for p in &paths {
                let target = resolve_cd(&shell.cwd, Path::new(p));
                let r = if parents {
                    fs::create_dir_all(&target)
                } else {
                    fs::create_dir(&target)
                };
                if let Err(e) = r {
                    eprintln!("mkdir: {}: {}", target.display(), e);
                    failed = true;
                }
            }

            if failed {
                return CmdOutcome::Done;
            }

            if enter {
                let target = resolve_cd(&shell.cwd, Path::new(paths[0]));
                return CmdOutcome::ChangeDir(target);
            }

            CmdOutcome::Done
        }
    }
}

fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if matches!(out.components().next_back(), Some(Component::Normal(_))) {
                    out.pop();
                } else if !out.has_root() {
                    out.push("..");
                }
            }
            _ => out.push(c.as_os_str()),
        }
    }
    out
}

fn resolve_cd(cwd: &Path, target: &Path) -> PathBuf {
    let joined = if target.is_absolute() {
        target.to_path_buf()
    } else {
        cwd.join(target)
    };
    normalize(&joined)
}

fn validate_dir(new_cwd: PathBuf) -> Option<PathBuf> {
    match std::fs::metadata(&new_cwd) {
        Ok(m) if m.is_dir() => Some(new_cwd),
        Ok(_) => {
            eprintln!("cd: {}: Not a directory", new_cwd.display());
            None
        }
        Err(e) => {
            let msg = match e.kind() {
                io::ErrorKind::NotFound => "No such file or directory".to_string(),
                io::ErrorKind::PermissionDenied => "Permission denied".to_string(),
                _ => e.to_string(),
            };
            eprintln!("cd: {}: {}", new_cwd.display(), msg);
            None
        }
    }
}

fn home_dir() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        std::env::var_os("USERPROFILE").map(PathBuf::from)
    }
    #[cfg(unix)]
    {
        std::env::var_os("HOME").map(PathBuf::from)
    }
    #[cfg(not(any(windows, unix)))]
    {
        None
    }
}

#[cfg(test)]
#[path = "../tests/unit/builtin.rs"]
mod tests;
