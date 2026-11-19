#[derive(Debug)]
pub enum Token {
    Word(String), // 普通词，引号已去掉
    Pipe,         // |
    RedirectOut,  // >
    Append,       // >>
    RedirectIn,   // <
}

pub fn lex(input: &str) -> Vec<Token> {
    let mut tokens: Vec<Token> = Vec::new();
    let mut cur = String::new();

    for ch in input.chars() {
        match ch {
            ' ' | '\t' | '\r' | '\n' => {
                if !cur.is_empty() {
                    tokens.push(Token::Word(std::mem::take(&mut cur)));
                }
            }
            '|' => {
                if !cur.is_empty() {
                    tokens.push(Token::Word(std::mem::take(&mut cur)));
                }
                tokens.push(Token::Pipe);
            }
            _ => cur.push(ch),
        }
    }
    if !cur.is_empty() {
        tokens.push(Token::Word(std::mem::take(&mut cur)));
    }
    tokens
}

#[cfg(test)]
#[path = "../tests/unit/lexer.rs"]
mod tests;
