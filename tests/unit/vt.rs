use crate::screen::Screen;
use crate::vt::Parser;

fn make_screen() -> Screen {
    // 测试用固定度量，不需要真字体
    Screen::new_test(20, 5)
}

fn line_text(screen: &Screen, row: usize) -> String {
    (0..screen.cols)
        .map(|x| screen.cells[row * screen.cols + x].ch)
        .collect::<String>()
        .trim_end()
        .to_string()
}

#[test]
fn plain_ascii_writes_to_cursor() {
    let mut screen = make_screen();
    let mut p = Parser::new();
    p.feed(b"hello", &mut screen);
    assert_eq!(line_text(&screen, 0), "hello");
    assert_eq!((screen.cx, screen.cy), (5, 0));
}

#[test]
fn crlf_moves_to_new_line() {
    let mut screen = make_screen();
    let mut p = Parser::new();
    p.feed(b"abc\r\ndef", &mut screen);
    assert_eq!(line_text(&screen, 0), "abc");
    assert_eq!(line_text(&screen, 1), "def");
}

#[test]
fn carriage_return_overwrites_current_line() {
    let mut screen = make_screen();
    let mut p = Parser::new();
    p.feed(b"hello\rHi", &mut screen);
    assert_eq!(line_text(&screen, 0), "Hillo");
}

#[test]
fn backspace_moves_cursor_left() {
    let mut screen = make_screen();
    let mut p = Parser::new();
    p.feed(b"abc\x08X", &mut screen);
    assert_eq!(line_text(&screen, 0), "abX");
}

#[test]
fn csi_sequences_are_consumed_not_printed() {
    let mut screen = make_screen();
    let mut p = Parser::new();
    // 这就是我们在窗口里看到的乱码 ^[[5;1R
    p.feed(b"\x1b[5;1Rok", &mut screen);
    assert_eq!(line_text(&screen, 0), "ok");
}

#[test]
fn csi_6n_sets_pending_dsr() {
    let mut screen = make_screen();
    let mut p = Parser::new();
    // cmd.exe 启动后的第一个输出，不回话就会卡住
    p.feed(b"\x1b[6n", &mut screen);
    assert!(p.pending_dsr);
    // 不影响网格内容
    assert_eq!(line_text(&screen, 0), "");
}

#[test]
fn osc_title_is_swallowed() {
    let mut screen = make_screen();
    let mut p = Parser::new();
    // ESC]0;标题 BEL —— 曾把标题文字当正文画上屏
    p.feed(b"\x1b]0;C:\\Windows\\system32\\cmd.exe\x07ok", &mut screen);
    assert_eq!(line_text(&screen, 0), "ok");
}

#[test]
fn csi_cursor_positioning() {
    let mut screen = make_screen();
    let mut p = Parser::new();
    // 第 3 行第 5 列
    p.feed(b"\x1b[3;5HX", &mut screen);
    assert_eq!(line_text(&screen, 2), "    X");
    assert_eq!((screen.cx, screen.cy), (5, 2));
}

#[test]
fn csi_home_by_empty_param() {
    let mut screen = make_screen();
    let mut p = Parser::new();
    p.feed(b"abc\r\ndef", &mut screen);
    p.feed(b"\x1b[H", &mut screen);
    assert_eq!((screen.cx, screen.cy), (0, 0));
}

#[test]
fn utf8_chinese_decoded() {
    let mut screen = make_screen();
    let mut p = Parser::new();
    // "你好" 的 UTF-8 编码；宽字符各占两格
    p.feed("你好".as_bytes(), &mut screen);
    assert_eq!(screen.cells[0].ch, '你');
    assert_eq!(screen.cells[1].ch, ' ');
    assert_eq!(screen.cells[2].ch, '好');
    assert_eq!((screen.cx, screen.cy), (4, 0));
}

#[test]
fn wide_char_wraps_instead_of_splitting() {
    let mut screen = make_screen();
    let mut p = Parser::new();
    // 屏幕宽 20：写 19 个 a 后光标在第 19 列，只剩 1 格放不下宽字符
    p.feed(&[b'a'; 19], &mut screen);
    p.feed("你".as_bytes(), &mut screen);
    assert_eq!(screen.cells[18].ch, 'a');
    assert_eq!(screen.cells[20].ch, '你');
    assert_eq!((screen.cx, screen.cy), (2, 1));
}

#[test]
fn utf8_split_across_feeds() {
    let mut screen = make_screen();
    let mut p = Parser::new();
    let bytes = "你".as_bytes();
    // 模拟 PTY 分片：一个字符被切成两次送达
    p.feed(&bytes[..1], &mut screen);
    p.feed(&bytes[1..], &mut screen);
    assert_eq!(screen.cells[0].ch, '你');
}

#[test]
fn csi_2j_clears_screen() {
    let mut screen = make_screen();
    let mut p = Parser::new();
    p.feed(b"junk\r\njunk2", &mut screen);
    p.feed(b"\x1b[2J", &mut screen);
    assert_eq!(line_text(&screen, 0), "");
    assert_eq!(line_text(&screen, 1), "");
    assert_eq!((screen.cx, screen.cy), (0, 0));
}

#[test]
fn wrap_at_line_end() {
    let mut screen = make_screen();
    let mut p = Parser::new();
    p.feed(b"aaaaaaaaaaaaaaaaaaaaaaaa", &mut screen); // 24 个 a，屏幕宽 20
    assert_eq!(line_text(&screen, 0), "aaaaaaaaaaaaaaaaaaaa");
    assert_eq!(line_text(&screen, 1), "aaaa");
}

#[test]
fn scroll_when_bottom_reached() {
    let mut screen = make_screen();
    let mut p = Parser::new();
    // 8 行文本 + 每行尾的换行，共 16 次换行；屏幕 5 行
    for i in 0..8 {
        p.feed(format!("line{i}\r\n").as_bytes(), &mut screen);
    }
    // 最后输入的是 "line7\r\n"，光标停在空行，所以倒数两行是 line7 和空行
    assert_eq!(line_text(&screen, 3), "line7");
    assert_eq!(line_text(&screen, 4), "");
}
