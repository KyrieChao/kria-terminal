use fontdue::{Font, FontSettings};
use minifb::{Window, WindowOptions};

const WIDTH: usize = 800;
const HEIGHT: usize = 600;
const FONT_SIZE: f32 = 32.0;
pub const BG_COLOR: u32 = 0x00202020;
pub const FG_COLOR: u32 = 0x00E0E0E0;

#[cfg(windows)]
const FONT_PATH: &str = r"C:\Windows\Fonts\consola.ttf";

/// 中文回退字体候选，按顺序取第一个能加载的
#[cfg(windows)]
const FALLBACK_PATHS: &[&str] = &[
    r"C:\Windows\Fonts\msyh.ttc",
    r"C:\Windows\Fonts\simhei.ttf",
    r"C:\Windows\Fonts\simkai.ttf",
    r"C:\Windows\Fonts\simfang.ttf",
];

/// 主字体 + 中文回退字体
pub struct FontSet {
    pub primary: Font,
    pub fallback: Option<Font>,
}

impl FontSet {
    pub fn load() -> Option<Self> {
        let primary = load_face(FONT_PATH)?;
        let mut fallback = None;
        for p in FALLBACK_PATHS {
            match load_face(p) {
                Some(f) => {
                    eprintln!("中文回退字体: {p}");
                    fallback = Some(f);
                    break;
                }
                None => eprintln!("回退字体 {p} 加载失败"),
            }
        }
        if fallback.is_none() {
            eprintln!("警告: 没找到中文回退字体，中文会显示为空白");
        }
        Some(Self { primary, fallback })
    }

    /// 主字体的字符表里没有该字符时改用回退字体。
    /// 不能用"位图为空"判断——缺字形时字体会返回 .notdef 方框，位图是非空的。
    pub fn rasterize(&self, ch: char, size: f32) -> (fontdue::Metrics, Vec<u8>) {
        if !self.primary.has_glyph(ch)
            && let Some(f) = &self.fallback
            && f.has_glyph(ch)
        {
            return f.rasterize(ch, size);
        }
        self.primary.rasterize(ch, size)
    }
}

fn load_face(path: &str) -> Option<Font> {
    let data = std::fs::read(path).ok()?;
    // .ttc 是字体集合，fontdue 只吃单个字体，取第一个 face 的数据段
    let face = if data.len() >= 16 && &data[0..4] == b"ttcf" {
        let off = u32::from_be_bytes(data[12..16].try_into().ok()?) as usize;
        data.get(off..)?
    } else {
        &data[..]
    };
    Font::from_bytes(face, FontSettings::default()).ok()
}

/// 东亚宽字符（CJK、假名、全角符号、emoji）在终端里占两格
pub fn is_wide(ch: char) -> bool {
    matches!(ch as u32,
        0x1100..=0x115F
        | 0x2E80..=0x303E
        | 0x3041..=0x33FF
        | 0x3400..=0x4DBF
        | 0x4E00..=0x9FFF
        | 0xA000..=0xA4CF
        | 0xAC00..=0xD7A3
        | 0xF900..=0xFAFF
        | 0xFE30..=0xFE6F
        | 0xFF00..=0xFF60
        | 0xFFE0..=0xFFE6
        | 0x1F300..=0x1F64F
        | 0x1F900..=0x1F9FF
        | 0x20000..=0x3FFFD
    )
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

    let fonts = FontSet::load().expect("字体加载失败");

    // 用字体度量反推一屏能放多少格
    let probe = crate::screen::Screen::new(1, 1, &fonts, FONT_SIZE);
    let cols = WIDTH / probe.cell_w;
    let rows = HEIGHT / probe.cell_h;
    let mut screen = crate::screen::Screen::new(cols, rows, &fonts, FONT_SIZE);

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
    let mut cmd = CommandBuilder::new("cmd.exe");
    // cmd 默认代码页是 GBK，中文输出不是 UTF-8，解析器会全部丢弃
    #[cfg(windows)]
    cmd.args(["/k", "chcp 65001 >nul"]);
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
    let start = std::time::Instant::now();

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
        let shift = shift_down(&window);
        let ctrl = ctrl_down();
        for key in window.get_keys_pressed(minifb::KeyRepeat::No) {
            if ctrl {
                match key {
                    minifb::Key::V => {
                        if let Some(text) = read_clipboard() {
                            // 终端里换行一律用 CR
                            input.push_str(&text.replace("\r\n", "\r").replace('\n', "\r"));
                        }
                        continue;
                    }
                    minifb::Key::C => {
                        input.push('\x03');
                        continue;
                    }
                    _ => {}
                }
            }
            input.push_str(key_to_str(key, shift));
        }
        if !input.is_empty() {
            let _ = writer.write_all(input.as_bytes());
            let _ = writer.flush();
            input.clear();
        }

        // 每 500ms 翻转一次，形成闪烁
        let blink = (start.elapsed().as_millis() / 500) % 2 == 0;
        screen.render(&mut buffer, WIDTH, HEIGHT, &fonts, blink);
        window.update_with_buffer(&buffer, WIDTH, HEIGHT).unwrap();
    }

    // 关窗口后 cmd.exe 不会自己退，直接杀掉再回收，否则 wait 会一直阻塞
    let _ = child.kill();
    let _ = child.wait();
}

/// 查 Win32 全局键状态。minifb 用 scancode 认键，部分键盘（尤其右 Shift）对不上。
#[cfg(windows)]
fn vk_down(vkey: i32) -> bool {
    // 只读键盘状态，无内存安全风险，仅需 unsafe 声明
    unsafe extern "system" {
        fn GetAsyncKeyState(vkey: i32) -> i16;
    }
    unsafe { (GetAsyncKeyState(vkey) as u16 & 0x8000) != 0 }
}

#[cfg(windows)]
fn shift_down(_window: &Window) -> bool {
    vk_down(0x10) // VK_SHIFT
}

#[cfg(windows)]
fn ctrl_down() -> bool {
    vk_down(0x11) // VK_CONTROL
}

#[cfg(not(windows))]
fn shift_down(window: &Window) -> bool {
    window.is_key_down(minifb::Key::LeftShift) || window.is_key_down(minifb::Key::RightShift)
}

#[cfg(not(windows))]
fn ctrl_down() -> bool {
    false
}

/// 读剪贴板里的 Unicode 文本。minifb 不支持 IME，中文只能靠粘贴进来。
#[cfg(windows)]
fn read_clipboard() -> Option<String> {
    // 剪贴板句柄由系统持有，这里只在锁定期间读取，不涉及内存归属
    unsafe extern "system" {
        fn OpenClipboard(hwnd: *mut core::ffi::c_void) -> i32;
        fn CloseClipboard() -> i32;
        fn GetClipboardData(format: u32) -> *mut core::ffi::c_void;
        fn GlobalLock(hmem: *mut core::ffi::c_void) -> *mut core::ffi::c_void;
        fn GlobalUnlock(hmem: *mut core::ffi::c_void) -> i32;
        fn GlobalSize(hmem: *mut core::ffi::c_void) -> usize;
    }
    const CF_UNICODETEXT: u32 = 13;

    unsafe {
        if OpenClipboard(std::ptr::null_mut()) == 0 {
            return None;
        }
        let handle = GetClipboardData(CF_UNICODETEXT);
        if handle.is_null() {
            CloseClipboard();
            return None;
        }
        let raw = GlobalLock(handle) as *const u16;
        if raw.is_null() {
            CloseClipboard();
            return None;
        }
        let cap = GlobalSize(handle) / 2;
        let mut len = 0;
        while len < cap && *raw.add(len) != 0 {
            len += 1;
        }
        let text = String::from_utf16_lossy(std::slice::from_raw_parts(raw, len));
        GlobalUnlock(handle);
        CloseClipboard();
        Some(text)
    }
}

#[cfg(not(windows))]
fn read_clipboard() -> Option<String> {
    None
}

/// minifb 只报物理键，Shift 组合的符号要自己换算
fn key_to_str(key: minifb::Key, shift: bool) -> &'static str {
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
        Key::A => shift_then(shift, "a", "A"),
        Key::B => shift_then(shift, "b", "B"),
        Key::C => shift_then(shift, "c", "C"),
        Key::D => shift_then(shift, "d", "D"),
        Key::E => shift_then(shift, "e", "E"),
        Key::F => shift_then(shift, "f", "F"),
        Key::G => shift_then(shift, "g", "G"),
        Key::H => shift_then(shift, "h", "H"),
        Key::I => shift_then(shift, "i", "I"),
        Key::J => shift_then(shift, "j", "J"),
        Key::K => shift_then(shift, "k", "K"),
        Key::L => shift_then(shift, "l", "L"),
        Key::M => shift_then(shift, "m", "M"),
        Key::N => shift_then(shift, "n", "N"),
        Key::O => shift_then(shift, "o", "O"),
        Key::P => shift_then(shift, "p", "P"),
        Key::Q => shift_then(shift, "q", "Q"),
        Key::R => shift_then(shift, "r", "R"),
        Key::S => shift_then(shift, "s", "S"),
        Key::T => shift_then(shift, "t", "T"),
        Key::U => shift_then(shift, "u", "U"),
        Key::V => shift_then(shift, "v", "V"),
        Key::W => shift_then(shift, "w", "W"),
        Key::X => shift_then(shift, "x", "X"),
        Key::Y => shift_then(shift, "y", "Y"),
        Key::Z => shift_then(shift, "z", "Z"),
        Key::Key0 => shift_then(shift, "0", ")"),
        Key::Key1 => shift_then(shift, "1", "!"),
        Key::Key2 => shift_then(shift, "2", "@"),
        Key::Key3 => shift_then(shift, "3", "#"),
        Key::Key4 => shift_then(shift, "4", "$"),
        Key::Key5 => shift_then(shift, "5", "%"),
        Key::Key6 => shift_then(shift, "6", "^"),
        Key::Key7 => shift_then(shift, "7", "&"),
        Key::Key8 => shift_then(shift, "8", "*"),
        Key::Key9 => shift_then(shift, "9", "("),
        Key::Minus => shift_then(shift, "-", "_"),
        Key::Equal => shift_then(shift, "=", "+"),
        Key::LeftBracket => shift_then(shift, "[", "{"),
        Key::RightBracket => shift_then(shift, "]", "}"),
        Key::Backslash => shift_then(shift, "\\", "|"),
        Key::Semicolon => shift_then(shift, ";", ":"),
        Key::Apostrophe => shift_then(shift, "'", "\""),
        Key::Comma => shift_then(shift, ",", "<"),
        Key::Period => shift_then(shift, ".", ">"),
        Key::Slash => shift_then(shift, "/", "?"),
        Key::Backquote => shift_then(shift, "`", "~"),
        _ => "",
    }
}

fn shift_then(shift: bool, plain: &'static str, shifted: &'static str) -> &'static str {
    if shift { shifted } else { plain }
}
