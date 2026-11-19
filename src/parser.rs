use crate::lexer::Token;

#[derive(Debug)]
pub struct Pipeline {
    pub commands: Vec<Command>,
}

#[derive(Debug)]
pub struct Command {
    pub program: String,
    pub args: Vec<String>,
}

#[derive(Debug)]
pub struct ParseError {
    pub message: String,
}

pub fn parse(tokens: &[Token]) -> Result<Pipeline, ParseError> {
    let mut commands = Vec::new();
    let mut current: Option<Command> = None;

    for token in tokens {
        match token {
            Token::Word(word) => match &mut current {
                Some(command) => command.args.push(word.clone()),
                None => {
                    current = Some(Command {
                        program: word.clone(),
                        args: Vec::new(),
                    });
                }
            },
            Token::Pipe => match current.take() {
                Some(command) => commands.push(command),
                None => {
                    return Err(ParseError {
                        message: "管道符前缺少命令".into(),
                    });
                }
            },
            _ => {
                return Err(ParseError {
                    message: "暂不支持重定向".into(),
                });
            }
        }
    }

    match current {
        Some(command) => commands.push(command),
        None => {
            return Err(ParseError {
                message: if commands.is_empty() {
                    "缺少命令".into()
                } else {
                    "管道符后缺少命令".into()
                },
            });
        }
    }
    Ok(Pipeline { commands })
}

#[cfg(test)]
#[path = "../tests/unit/parser.rs"]
mod tests;
