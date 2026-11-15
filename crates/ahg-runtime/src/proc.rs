//! Starting, reading and stopping child processes.
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::thread;
use std::time::{Duration, Instant};

/// Which pipe a chunk of output came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stream {
    Stdout,
    Stderr,
}

/// A command that opens no console window and has both output pipes captured.
pub fn hidden(program: &std::path::Path) -> Command {
    let mut cmd = Command::new(program);
    cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

/// Reads both pipes of `child` on their own threads. The receiver yields raw
/// chunks in arrival order and ends when both pipes have closed.
pub fn read_output(child: &mut Child) -> Receiver<(Stream, Vec<u8>)> {
    let (tx, rx) = channel();
    let pump = |stream: Stream, pipe: Option<Box<dyn Read + Send>>| {
        let Some(mut pipe) = pipe else { return };
        let tx = tx.clone();
        thread::spawn(move || {
            let mut buf = [0u8; 8192];
            while let Ok(n) = pipe.read(&mut buf) {
                if n == 0 || tx.send((stream, buf[..n].to_vec())).is_err() {
                    break;
                }
            }
        });
    };
    pump(Stream::Stdout, child.stdout.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
    pump(Stream::Stderr, child.stderr.take().map(|p| Box::new(p) as Box<dyn Read + Send>));
    rx
}

/// Turns chunks of UTF-8 into text, holding back a character that was cut in
/// half at the end of a chunk until the rest of it arrives. Bytes that are not
/// UTF-8 at all become the replacement character.
#[derive(Default)]
pub struct Utf8Chunks {
    pending: Vec<u8>,
}

impl Utf8Chunks {
    pub fn push(&mut self, chunk: &[u8]) -> String {
        self.pending.extend_from_slice(chunk);
        let mut out = String::new();
        loop {
            match std::str::from_utf8(&self.pending) {
                Ok(text) => {
                    out.push_str(text);
                    self.pending.clear();
                    return out;
                }
                Err(e) => {
                    let valid = e.valid_up_to();
                    out.push_str(&String::from_utf8_lossy(&self.pending[..valid]));
                    match e.error_len() {
                        // An incomplete character at the very end: keep it for the next chunk.
                        None => {
                            self.pending.drain(..valid);
                            return out;
                        }
                        Some(bad) => {
                            out.push('\u{FFFD}');
                            self.pending.drain(..valid + bad);
                        }
                    }
                }
            }
        }
    }

    /// What is left when the stream ends.
    pub fn finish(&mut self) -> String {
        String::from_utf8_lossy(&std::mem::take(&mut self.pending)).into_owned()
    }
}

/// Waits for `child` to exit, for at most `limit`. `None` means it is still running.
pub fn wait_for(child: &mut Child, limit: Duration) -> Option<std::process::ExitStatus> {
    let deadline = Instant::now() + limit;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            _ => return None,
        }
    }
}

/// Kills the process with this id and everything it started. Killing only the
/// process itself would leave yt-dlp's ffmpeg child running.
pub fn kill_tree(pid: u32) {
    #[cfg(windows)]
    {
        let mut cmd = hidden(std::path::Path::new("taskkill"));
        cmd.args(["/pid", &pid.to_string(), "/T", "/F"]).stdout(Stdio::null()).stderr(Stdio::null());
        if let Ok(mut killer) = cmd.spawn()
            && wait_for(&mut killer, Duration::from_secs(5)).is_none()
        {
            let _ = killer.kill();
        }
    }
    #[cfg(not(windows))]
    {
        let _ = Command::new("kill").args(["-9", &pid.to_string()]).status();
    }
}
