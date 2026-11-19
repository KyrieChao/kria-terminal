use crate::Shell;
use crate::parser::Command;
use std::io::{self, Write};
use std::path::{Component, Path, PathBuf};

pub enum BuiltinResult {
    Continue,
    Exit,
    NotFound,
}

pub fn dispatch(shell: &mut Shell, cmd: &Command) -> BuiltinResult {
    match cmd.program.as_str() {
        "exit" => {
            println!("bye");
            BuiltinResult::Exit
        }
        "pwd" => {
            println!("{}", shell.cwd.display());
            BuiltinResult::Continue
        }
        "cd" => {
            handle_cd(shell, cmd);
            BuiltinResult::Continue
        }
        "clear" => {
            let mut stdout = io::stdout().lock();
            if let Err(error) = stdout
                .write_all(b"\x1b[2J\x1b[H")
                .and_then(|()| stdout.flush())
            {
                eprintln!("clear: {error}");
            }
            BuiltinResult::Continue
        }
        "ls" => {
            let result = std::fs::read_dir(&shell.cwd);
            match result {
                Ok(entries) => {
                    for entry in entries {
                        match entry {
                            Ok(entry) => println!("{}", entry.file_name().to_string_lossy()),
                            Err(e) => eprintln!("读取条目失败：{}", e),
                        }
                    }
                }
                Err(e) => eprintln!("打开失败：{}", e),
            }
            BuiltinResult::Continue
        }
        _ => BuiltinResult::NotFound,
    }
}

fn handle_cd(shell: &mut Shell, cmd: &Command) {
    match cmd.args.first().map(String::as_str) {
        Some("-") => {
            let new_cwd = match shell.prev.as_ref() {
                Some(p) => resolve_cd(&shell.cwd, p),
                None => {
                    eprintln!("cd: OLDPWD not set");
                    return;
                }
            };
            if commit_cd(shell, new_cwd) {
                println!("{}", shell.cwd.display());
            }
        }
        Some(arg) => {
            let new_cwd = resolve_cd(&shell.cwd, Path::new(arg));
            commit_cd(shell, new_cwd);
        }
        None => {
            if cfg!(windows) {
                println!("{}", shell.cwd.display());
            } else if let Some(home) = home_dir() {
                let new_cwd = resolve_cd(&shell.cwd, &home);
                commit_cd(shell, new_cwd);
            } else {
                eprintln!("cd: HOME not set");
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

fn commit_cd(shell: &mut Shell, new_cwd: PathBuf) -> bool {
    match std::fs::metadata(&new_cwd) {
        Ok(m) if m.is_dir() => {
            shell.prev = Some(std::mem::replace(&mut shell.cwd, new_cwd));
            true
        }
        Ok(_) => {
            eprintln!("cd: {}: Not a directory", new_cwd.display());
            false
        }
        Err(e) => {
            let msg = match e.kind() {
                io::ErrorKind::NotFound => "No such file or directory".to_string(),
                io::ErrorKind::PermissionDenied => "Permission denied".to_string(),
                _ => e.to_string(),
            };
            eprintln!("cd: {}: {}", new_cwd.display(), msg);
            false
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
