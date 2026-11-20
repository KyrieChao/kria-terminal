use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::io::{Read, Write};

pub fn run() {
    let pty_system = native_pty_system();

    let pair = pty_system
        .openpty(PtySize {
            rows: 30,
            cols: 100,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("打开 PTY 失败");

    #[cfg(windows)]
    let mut cmd = CommandBuilder::new("cmd.exe");
    #[cfg(not(windows))]
    let mut cmd = CommandBuilder::new("bash");

    cmd.cwd(std::env::current_dir().unwrap_or_default());

    let mut child = pair.slave.spawn_command(cmd).expect("启动 shell 失败");
    // slave 必须 drop，否则 PTY 不会收到 EOF
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().expect("获取读端失败");
    let mut writer = pair.master.take_writer().expect("获取写端失败");

    // PTY 的读是阻塞的，放在独立线程里避免卡住主流程
    let reader_thread = std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let text = String::from_utf8_lossy(&buf[..n]);
                    print!("{text}");
                    let _ = std::io::stdout().flush();
                }
                Err(_) => break,
            }
        }
    });

    // 把 stdin 转发给 PTY，直到 Ctrl+Z / EOF
    std::thread::spawn(move || {
        let mut input = [0u8; 1024];
        loop {
            match std::io::stdin().read(&mut input) {
                Ok(0) => break,
                Ok(n) => {
                    if writer.write_all(&input[..n]).is_err() {
                        break;
                    }
                    let _ = writer.flush();
                }
                Err(_) => break,
            }
        }
    });

    let _ = reader_thread.join();
    let _ = child.wait();
}
