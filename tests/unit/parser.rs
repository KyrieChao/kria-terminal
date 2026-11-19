use super::parse;
use crate::lexer::{Token, lex};

#[test]
fn parses_pipeline_and_arguments() {
    let pipeline = parse(&lex("ls -l | grep demo")).unwrap();

    assert_eq!(pipeline.commands.len(), 2);
    assert_eq!(pipeline.commands[0].program, "ls");
    assert_eq!(pipeline.commands[0].args, ["-l"]);
    assert_eq!(pipeline.commands[1].program, "grep");
    assert_eq!(pipeline.commands[1].args, ["demo"]);
}

#[test]
fn rejects_missing_commands() {
    for input in ["", "| ls", "ls | | grep", "ls |"] {
        assert!(parse(&lex(input)).is_err(), "{input:?}");
    }
}

#[test]
fn rejects_unsupported_redirections() {
    for token in [Token::RedirectOut, Token::Append, Token::RedirectIn] {
        let tokens = [Token::Word("ls".into()), token];
        assert_eq!(parse(&tokens).unwrap_err().message, "暂不支持重定向");
    }
}
