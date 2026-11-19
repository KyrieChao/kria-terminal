use super::*;

#[test]
fn simple_words() {
    // "ls -l" → ["ls", "-l"]
    let tokens = lex("ls -l");
    assert_eq!(tokens.len(), 2);
    assert!(matches!(&tokens[0], Token::Word(word) if word == "ls"));
    assert!(matches!(&tokens[1], Token::Word(word) if word == "-l"));
}

#[test]
fn pipe_no_spaces() {
    // "ls|grep" → ["ls", "|", "grep"]
    let tokens = lex("ls|grep ");
    assert_eq!(tokens.len(), 3);
    assert!(matches!(&tokens[0], Token::Word(word) if word == "ls"));
    assert!(matches!(&tokens[1], Token::Pipe));
    assert!(matches!(&tokens[2], Token::Word(word) if word == "grep"));
}

#[test]
fn pipe_with_spaces() {
    // "ls | grep x" → ["ls", "|", "grep", "x"]
    let tokens = lex("ls | grep x");
    assert_eq!(tokens.len(), 4);
    assert!(matches!(&tokens[0], Token::Word(word) if word == "ls"));
    assert!(matches!(&tokens[1], Token::Pipe));
    assert!(matches!(&tokens[2], Token::Word(word) if word == "grep"));
    assert!(matches!(&tokens[3], Token::Word(word) if word == "x"));
}

#[test]
fn trailing_pipe() {
    // "ls |" → ["ls", "|"]   不能有空的 Word
    let tokens = lex("ls |");
    assert_eq!(tokens.len(), 2);
    assert!(matches!(&tokens[0], Token::Word(word) if word == "ls"));
    assert!(matches!(&tokens[1], Token::Pipe));
}

#[test]
fn trailing_newline() {
    // "ls\r\n" → ["ls"]      不能有 \r\n
    let tokens = lex("ls\r\n");
    assert_eq!(tokens.len(), 1);
    assert!(matches!(&tokens[0], Token::Word(word) if word == "ls"));
}

#[test]
fn empty_input() {
    // "" → []
    let tokens = lex("");
    assert_eq!(tokens.len(), 0);
}
