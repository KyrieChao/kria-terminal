use fontdue::{Font, FontSettings};
use minifb::{Window, WindowOptions};

const WIDTH: usize = 800;
const HEIGHT: usize = 600;
const FONT_SIZE: f32 = 32.0;
pub const BG_COLOR: u32 = 0x00202020;
pub const FG_COLOR: u32 = 0x00E0E0E0;

#[cfg(windows)]
const FONT_PATH: &str = r"C:\Windows\Fonts\consola.ttf";

fn load_font() -> Option<Font> {
    let data = std::fs::read(FONT_PATH).ok()?;
    Font::from_bytes(data, FontSettings::default()).ok()
}

/// 按字形覆盖率把前景色混合到背景上（alpha blending）
fn mix(bg: u32, fg: u32, a: u32) -> u32 {
    (bg * (255 - a) + fg * a) / 255
}

/// 把 fontdue 产出的灰度位图（覆盖度 0-255）混合绘制到窗口缓冲
#[allow(clippy::too_many_arguments)]
pub fn draw_glyph(
    buffer: &mut [u32],
    width: usize,
    height: usize,
    glyph: &[u8],
    gw: usize,
    gh: usize,
    px: isize,
    py: isize,
    color: u32,
) {
    let (r_fg, g_fg, b_fg) = (
        (color >> 16) & 0xFF,
        (color >> 8) & 0xFF,
        color & 0xFF,
    );

    for gy in 0..gh {
        let y = py + gy as isize;
        if y < 0 || y >= height as isize {
            continue;
        }
        for gx in 0..gw {
            let x = px + gx as isize;
            if x < 0 || x >= width as isize {
                continue;
            }

            let a = glyph[gy * gw + gx] as u32;
            if a == 0 {
                continue;
            }

            let idx = y as usize * width + x as usize;
            let bg = buffer[idx];
            let r = mix((bg >> 16) & 0xFF, r_fg, a);
            let g = mix((bg >> 8) & 0xFF, g_fg, a);
            let b = mix(bg & 0xFF, b_fg, a);
            buffer[idx] = (r << 16) | (g << 8) | b;
        }
    }
}

pub fn run() {
    use portable_pty::{CommandBuilder, PtySize, native_pty_system};
    use std::io::{Read, Write};
    use std::sync::mpsc;

    let font = load_font().expect("字体加载失败");

    // 用字体度量反推一屏能放多少格
    let probe = crate::screen::Screen::new(1, 1, &font, FONT_SIZE);
    let cols = WIDTH / probe.cell_w;
    let rows = HEIGHT / probe.cell_h;
    let mut screen = crate::screen::Screen::new(cols, rows, &font, FONT_SIZE);

    let pty_system = native_pty_system();
    let pair = pty_system
        .openpty(PtySize {
            rows: rows as u16,
            cols: cols as u16,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("打开 PTY 失败");

    #[cfg(windows)]
    let cmd = CommandBuilder::new("cmd.exe");
    #[cfg(not(windows))]
    let cmd = CommandBuilder::new("bash");

    let mut child = pair.slave.spawn_command(cmd).expect("启动 shell 失败");
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().expect("获取读端失败");
    let mut writer = pair.master.take_writer().expect("获取写端失败");

    // PTY 读取是阻塞的，用线程把字节送进通道
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
            }
        }
    });

    let mut window = Window::new("kria-terminal", WIDTH, HEIGHT, WindowOptions::default()).unwrap();
    window.set_target_fps(60);

    let mut buffer = vec![BG_COLOR; WIDTH * HEIGHT];
    let mut parser = crate::vt::Parser::new();
    let mut input = String::new();

    while window.is_open() {
        // 把 PTY 这段时间收到的字节全部喂给解析器
        while let Ok(bytes) = rx.try_recv() {
            parser.feed(&bytes, &mut screen);
        }
        // 程序问光标位置，必须回，否则它会一直等
        if parser.pending_dsr {
            parser.pending_dsr = false;
            let reply = format!("\x1b[{};{}R", screen.cy + 1, screen.cx + 1);
            let _ = writer.write_all(reply.as_bytes());
            let _ = writer.flush();
        }

        // 键盘输入转发给 PTY
        for key in window.get_keys_pressed(minifb::KeyRepeat::No) {
            input.push_str(key_to_str(key));
        }
        if !input.is_empty() {
            let _ = writer.write_all(input.as_bytes());
            let _ = writer.flush();
            input.clear();
        }

        screen.render(&mut buffer, WIDTH, HEIGHT, &font);
        window.update_with_buffer(&buffer, WIDTH, HEIGHT).unwrap();
    }

    let _ = child.wait();
}

fn key_to_str(key: minifb::Key) -> &'static str {
    use minifb::Key;
    match key {
        Key::Enter => "\r",
        Key::Backspace => "\x7f",
        Key::Tab => "\t",
        Key::Escape => "\x1b",
        Key::Up => "\x1b[A",
        Key::Down => "\x1b[B",
        Key::Right => "\x1b[C",
        Key::Left => "\x1b[D",
        Key::Space => " ",
        Key::A => "a", Key::B => "b", Key::C => "c", Key::D => "d", Key::E => "e",
        Key::F => "f", Key::G => "g", Key::H => "h", Key::I => "i", Key::J => "j",
        Key::K => "k", Key::L => "l", Key::M => "m", Key::N => "n", Key::O => "o",
        Key::P => "p", Key::Q => "q", Key::R => "r", Key::S => "s", Key::T => "t",
        Key::U => "u", Key::V => "v", Key::W => "w", Key::X => "x", Key::Y => "y",
        Key::Z => "z",
        Key::Key0 => "0", Key::Key1 => "1", Key::Key2 => "2", Key::Key3 => "3",
        Key::Key4 => "4", Key::Key5 => "5", Key::Key6 => "6", Key::Key7 => "7",
        Key::Key8 => "8", Key::Key9 => "9",
        Key::Minus => "-", Key::Equal => "=", Key::Period => ".", Key::Comma => ",",
        Key::Slash => "/", Key::Backslash => "\\", Key::Semicolon => ";",
        Key::Apostrophe => "'", Key::LeftBracket => "[", Key::RightBracket => "]",
        Key::Backquote => "`",
        _ => "",
    }
}
