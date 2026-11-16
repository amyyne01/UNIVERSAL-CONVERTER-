//! A scripted stand-in for yt-dlp, used only by this crate's tests. It never
//! touches the network.
//!
//! `--version` prints a version and exits 0 (or exits 1 when
//! `FAKE_YTDLP_BROKEN` is set). Otherwise it plays the script file named by
//! `FAKE_YTDLP_SCRIPT`, one instruction per line:
//!
//! - `out <text>` / `err <text>`: write the text and a newline to stdout / stderr
//! - `outhex <hex>` / `errhex <hex>`: write raw bytes, with no newline
//! - `sleep <ms>`
//! - `child <file>`: start a copy of itself that sleeps for a minute, and write its pid to the file
//! - `count <file>`: append a line to the file, to count how often the script ran
//! - `pid <file>`: append this process's own id as a line to the file
//! - `failonce <file>`: the first run (the file is missing) prints a network error and exits 1
//! - `exit <code>`
//!
//! A call with `--dump-json` (a metadata or search call) plays
//! `FAKE_YTDLP_SCRIPT_JSON` instead, when that is set.
//!
//! When `FAKE_YTDLP_ARGS` names a file, the arguments are written there, one per
//! line (to `<file>.json` for a `--dump-json` call).
use std::io::Write;
use std::process::{Command, exit};
use std::time::Duration;

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2).filter_map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).ok()).collect()
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let inspect = args.iter().any(|a| a == "--dump-json");
    if let Ok(file) = std::env::var("FAKE_YTDLP_ARGS") {
        let _ = std::fs::write(if inspect { format!("{file}.json") } else { file }, args.join("\n"));
    }
    if args.iter().any(|a| a == "--version") {
        if std::env::var_os("FAKE_YTDLP_BROKEN").is_some() {
            exit(1);
        }
        println!("2026.01.01");
        return;
    }
    let for_json = std::env::var("FAKE_YTDLP_SCRIPT_JSON").ok().filter(|_| inspect);
    let script =
        for_json.or_else(|| std::env::var("FAKE_YTDLP_SCRIPT").ok()).and_then(|f| std::fs::read_to_string(f).ok()).unwrap_or_default();
    let (mut out, mut err) = (std::io::stdout(), std::io::stderr());
    for line in script.lines() {
        let (op, rest) = line.split_once(' ').unwrap_or((line, ""));
        match op {
            "out" => {
                let _ = writeln!(out, "{rest}");
                let _ = out.flush();
            }
            "err" => {
                let _ = writeln!(err, "{rest}");
                let _ = err.flush();
            }
            "outhex" => {
                let _ = out.write_all(&unhex(rest));
                let _ = out.flush();
            }
            "errhex" => {
                let _ = err.write_all(&unhex(rest));
                let _ = err.flush();
            }
            "sleep" => std::thread::sleep(Duration::from_millis(rest.parse().unwrap_or(0))),
            "child" => {
                let sleeper = std::env::temp_dir().join(format!("fake-ytdlp-sleeper-{}.txt", std::process::id()));
                let _ = std::fs::write(&sleeper, "sleep 60000\n");
                if let Ok(exe) = std::env::current_exe()
                    && let Ok(child) = Command::new(exe).env("FAKE_YTDLP_SCRIPT", &sleeper).env_remove("FAKE_YTDLP_ARGS").spawn()
                {
                    let _ = std::fs::write(rest, child.id().to_string());
                }
            }
            "pid" => {
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(rest) {
                    let _ = f.write_all(
                        format!(
                            "{}
",
                            std::process::id()
                        )
                        .as_bytes(),
                    );
                }
            }
            "count" => {
                if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(rest) {
                    let _ = f.write_all(
                        b"run
",
                    );
                }
            }
            "failonce" => {
                if !std::path::Path::new(rest).exists() {
                    let _ = std::fs::write(rest, "failed once");
                    let _ = writeln!(err, "ERROR: Connection reset by peer");
                    exit(1);
                }
            }
            "exit" => exit(rest.parse().unwrap_or(1)),
            _ => {}
        }
    }
}
