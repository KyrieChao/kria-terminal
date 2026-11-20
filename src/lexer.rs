#[derive(Debug)]
pub enum Token {
    Word(String), // 普通词 引号已去掉
    Pipe,         // |
    RedirectOut,  // >
    Append,       // >>
    RedirectIn,   // <
}

fn push_word(mut curr: &mut String, tokens: &mut Vec<Token>) {
    if !curr.is_empty() {
        tokens.push(Token::Word(std::mem::take(&mut curr)));
    }
}

pub fn lex(input: &str) -> Vec<Token> {
    let mut tokens: Vec<Token> = Vec::new();
    let mut cur = String::new();
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            ' ' | '\t' | '\r' | '\n' => {
                push_word(&mut cur, &mut tokens);
            }
            '|' => {
                push_word(&mut cur, &mut tokens);
                tokens.push(Token::Pipe);
            }
            '>' => {
                push_word(&mut cur, &mut tokens);
                if chars.peek() == Some(&'>') {
                    chars.next();
                    tokens.push(Token::Append);
                } else {
                    tokens.push(Token::RedirectOut);
                }
            }
            '<' => {
                push_word(&mut cur, &mut tokens);
                tokens.push(Token::RedirectIn);
            }
            _ => cur.push(ch),
        }
    }
    push_word(&mut cur, &mut tokens);
    tokens
}

#[cfg(test)]
#[path = "../tests/unit/lexer.rs"]
mod tests;
