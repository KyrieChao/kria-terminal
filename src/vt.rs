use crate::screen::Screen;

#[cfg(test)]
#[path = "../tests/unit/vt.rs"]
mod tests;

/// 最小 VT 解析器：把字节流转换成对屏幕网格的操作。
pub struct Parser {
    state: State,
    csi: String,
    /// 程序发来的 DSR 请求（ESC[6n），需要终端回一个光标位置
    pub pending_dsr: bool,
    /// UTF-8 多字节字符的拼接缓冲
    utf8: Vec<u8>,
}

enum State {
    Ground,
    Escape,
    Csi,
    /// OSC 序列 `ESC ] ... BEL`（常见于 `ESC]0;标题 BEL`）
    Osc,
}

impl Parser {
    pub fn new() -> Self {
        Self {
            state: State::Ground,
            csi: String::new(),
            pending_dsr: false,
            utf8: Vec::new(),
        }
    }

    pub fn feed(&mut self, bytes: &[u8], screen: &mut Screen) {
        for &b in bytes {
            self.feed_byte(b, screen);
        }
    }

    fn feed_byte(&mut self, b: u8, screen: &mut Screen) {
        match self.state {
            State::Ground => match b {
                0x1b => self.state = State::Escape,
                b'\n' => screen.line_feed(),
                b'\r' => screen.carriage_return(),
                0x08 => screen.backspace(),
                b'\t' => screen.write_char(' '),
                0x20..=0x7e => screen.write_char(b as char),
                // 高位字节：可能是 UTF-8 多字节字符的开头或续字节
                _ => self.feed_utf8(b, screen),
            },
            State::Escape => match b {
                b'[' => {
                    self.csi.clear();
                    self.state = State::Csi;
                }
                // OSC：ESC ] ... BEL
                b']' => self.state = State::Osc,
                // 其他两字符转义序列（如 ESC =）：直接回 Ground
                _ => self.state = State::Ground,
            },
            State::Csi => match b {
                // 最终字节 0x40-0x7E 表示序列结束
                0x40..=0x7e => {
                    self.handle_csi(b, screen);
                    self.state = State::Ground;
                }
                _ => self.csi.push(b as char),
            },
            State::Osc => match b {
                // BEL 或 ESC 都表示 OSC 结束
                0x07 | 0x1b => self.state = State::Ground,
                // 标题文字等内容直接丢弃，不上屏
                _ => {}
            },
        }
    }

    /// 累积 UTF-8 字节，凑成一个完整字符后写到屏幕上
    fn feed_utf8(&mut self, b: u8, screen: &mut Screen) {
        self.utf8.push(b);
        match std::str::from_utf8(&self.utf8) {
            Ok(s) => {
                for ch in s.chars() {
                    screen.write_char(ch);
                }
                self.utf8.clear();
            }
            // 字节还不够长，继续等
            Err(e) if e.error_len().is_none() => {}
            // 真的非法：丢掉缓冲，避免污染后续
            Err(_) => self.utf8.clear(),
        }
    }

    fn handle_csi(&mut self, final_byte: u8, screen: &mut Screen) {
        match final_byte {
            // 清屏
            b'J' if self.csi == "2" => screen.clear(),
            // 光标定位：CSI {row};{col} H，空参数默认 1
            b'H' => {
                let (row, col) = parse_cursor_pos(&self.csi);
                screen.move_to(col, row);
            }
            // 设备状态报告：程序问"光标在哪"，必须回复否则程序会卡住
            b'n' if self.csi == "6" => self.pending_dsr = true,
            _ => {}
        }
    }
}

/// 解析 CSI 的参数部分 `"r;c"`，返回 0 基的 (row, col)
fn parse_cursor_pos(params: &str) -> (usize, usize) {
    let mut parts = params.split(';');
    let row = parse_param(parts.next());
    let col = parse_param(parts.next());
    (row, col)
}

/// CSI 参数从 1 开始计数，空参数按 1 处理；转成 0 基
fn parse_param(s: Option<&str>) -> usize {
    s.and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(1)
        .saturating_sub(1)
}
