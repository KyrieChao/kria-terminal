use crate::ShellSnapshot;
use crate::parser::Command;
use std::fs;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

pub enum CmdOutcome {
    Done,
    ChangeDir(PathBuf), // 请求改到某个目录，由调用方决定应用与否
    Exit,
}

pub fn is_builtin(name: &str) -> bool {
    matches!(name, "exit" | "pwd" | "cd" | "clear" | "ls" | "mkdir")
}

pub fn run_builtin(
    shell: &ShellSnapshot,
    cmd: &Command,
    stdout: &mut dyn Write,
) -> Option<CmdOutcome> {
    match cmd.program.as_str() {
        "exit" => {
            let _ = writeln!(stdout, "bye");
            Some(CmdOutcome::Exit)
        }
        "pwd" => {
            let _ = writeln!(stdout, "{}", shell.cwd.display());
            Some(CmdOutcome::Done)
        }
        "cd" => match handle_cd(shell, cmd, stdout) {
            Some(new_cwd) => Some(CmdOutcome::ChangeDir(new_cwd)),
            None => Some(CmdOutcome::Done),
        },
        "clear" => {
            let _ = stdout.write_all(b"\x1b[2J\x1b[H");
            let _ = stdout.flush();
            Some(CmdOutcome::Done)
        }
        "ls" => {
            let result = std::fs::read_dir(&shell.cwd);
            match result {
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
            Some(CmdOutcome::Done)
        }
        "mkdir" => {
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
                                    return Some(CmdOutcome::Done);
                                }
                            }
                        }
                    }
                    _ => paths.push(arg.as_str()),
                }
            }
            if paths.is_empty() {
                eprintln!("mkdir: missing operand");
                return Some(CmdOutcome::Done);
            }
            if enter && paths.len() > 1 {
                eprintln!("mkdir: -e requires exactly one path");
                return Some(CmdOutcome::Done);
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
                return Some(CmdOutcome::Done);
            }
            if enter {
                return Some(CmdOutcome::ChangeDir(resolve_cd(
                    &shell.cwd,
                    Path::new(paths[0]),
                )));
            }
            Some(CmdOutcome::Done)
        }
        _ => None,
    }
}

fn handle_cd(shell: &ShellSnapshot, cmd: &Command, stdout: &mut dyn Write) -> Option<PathBuf> {
    match cmd.args.first().map(String::as_str) {
        Some("-") => {
            let prev = shell.prev.as_ref()?;
            let new_cwd = resolve_cd(&shell.cwd, prev);
            validate_dir(new_cwd)
        }
        Some(arg) => {
            let new_cwd = resolve_cd(&shell.cwd, Path::new(arg));
            validate_dir(new_cwd)
        }
        None => {
            if cfg!(windows) {
                let _ = writeln!(stdout, "{}", shell.cwd.display());
                None
            } else if let Some(home) = home_dir() {
                let new_cwd = resolve_cd(&shell.cwd, &home);
                validate_dir(new_cwd)
            } else {
                eprintln!("cd: HOME not set");
                None
            }
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
