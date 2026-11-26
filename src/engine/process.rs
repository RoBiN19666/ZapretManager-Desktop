//! Запуск внешних программ с захватом вывода и таймаутом.

use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

/// CREATE_NO_WINDOW — не показывать консольное окно.
pub const CREATE_NO_WINDOW: u32 = 0x0800_0000;

#[derive(Debug, Clone, Default)]
pub struct ProcResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

impl ProcResult {
    pub fn all(&self) -> String {
        let mut s = self.stdout.clone();
        if !self.stderr.trim().is_empty() {
            if !s.is_empty() {
                s.push_str("\r\n");
            }
            s.push_str(&self.stderr);
        }
        s.trim().to_string()
    }
}

/// Новая команда без консольного окна.
pub fn command(exe: &str) -> Command {
    let mut cmd = Command::new(exe);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);
    cmd
}

fn read_stream<R: Read + Send + 'static>(mut r: R) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut buf = Vec::new();
        let _ = r.read_to_end(&mut buf);
        buf
    })
}

/// Запустить программу, дождаться выхода (не дольше timeout) и вернуть вывод.
pub fn run(exe: &str, args: &[&str], timeout: Duration) -> ProcResult {
    run_with_env(exe, args, None, &[], timeout)
}

/// То же, но с рабочей папкой и переменными окружения.
pub fn run_with_env(
    exe: &str,
    args: &[&str],
    cwd: Option<&Path>,
    envs: &[(&str, String)],
    timeout: Duration,
) -> ProcResult {
    let mut cmd = command(exe);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    for (key, value) in envs {
        cmd.env(key, value);
    }
    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            return ProcResult {
                exit_code: -1,
                stdout: String::new(),
                stderr: e.to_string(),
            }
        }
    };
    let out = child.stdout.take().map(read_stream);
    let err = child.stderr.take().map(read_stream);

    let deadline = Instant::now() + timeout;
    let mut code: Option<i32> = None;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                code = Some(status.code().unwrap_or(-1));
                break;
            }
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    break;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(_) => break,
        }
    }
    let stdout = out
        .and_then(|h| h.join().ok())
        .map(|b| crate::engine::textfile::decode_bytes(&b))
        .unwrap_or_default();
    let stderr = err
        .and_then(|h| h.join().ok())
        .map(|b| crate::engine::textfile::decode_bytes(&b))
        .unwrap_or_default();
    ProcResult {
        exit_code: code.unwrap_or(-1),
        stdout,
        stderr,
    }
}

/// Запустить `cmd.exe /c <команда>` — для команд с перенаправлениями и кавычками.
pub fn run_shell(command_line: &str, timeout: Duration) -> ProcResult {
    run("cmd.exe", &["/c", command_line], timeout)
}

/// Запустить процесс скрыто и не ждать его (фоновый режим).
pub fn spawn_hidden(exe: &str, args: &[&str], cwd: Option<&Path>) -> std::io::Result<Child> {
    let mut cmd = command(exe);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    cmd.spawn()
}

/// Запустить процесс с захватом stdout/stderr; вызывающий читает потоки сам.
pub fn spawn_captured(exe: &str, args: &[&str], cwd: Option<&Path>) -> std::io::Result<Child> {
    let mut cmd = command(exe);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    cmd.spawn()
}

/// Жив ли процесс с таким PID.
pub fn is_alive(pid: u32) -> bool {
    super::sys::list_processes().iter().any(|p| p.pid == pid)
}

/// Подождать, пока функция не вернёт true (или истечёт таймаут).
pub fn wait_until<const N: usize>(timeout: Duration, mut check: impl FnMut() -> bool) -> bool {
    let _ = N;
    let deadline = Instant::now() + timeout;
    loop {
        if check() {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

/// Запустить длительную операцию в отдельном потоке и вернуть получатель результата.
pub fn spawn_with_result<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> mpsc::Receiver<T> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        let _ = tx.send(work());
    });
    rx
}
