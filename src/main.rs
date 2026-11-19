mod builtin;
mod lexer;
mod parser;
use std::io;
use std::io::Write;
use std::path::PathBuf;

use crate::builtin::BuiltinResult::Continue;

pub struct Shell {
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
}

fn main() {
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
                        eprintln!("暂不支持管道");
                        continue;
                    }
                };
                match builtin::dispatch(&mut shell, cmd) {
                    builtin::BuiltinResult::Exit => break,
                    builtin::BuiltinResult::NotFound => {
                        println!("未实现的命令: {}", cmd.program);
                    }
                    builtin::BuiltinResult::Continue => {}
                }
            }
            Err(e) => {
                eprintln!("读输入失败: {e}");
                break;
            }
        }
    }
}
