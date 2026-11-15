//! Discord Rich Presence: advertises how many downloads run and wait on the
//! user's chat profile. Ported from electron/presence.ts. This is the decision
//! logic only. The local pipe, the two timers and the process id belong to the
//! app layer; it feeds events in and carries out the `Effect`s that come back.
//!
//! Events to feed: `start`, `set_activity`, `stop`, `on_connect` (the probe
//! socket connected), `on_probe_error(i)` (a probe socket failed before
//! connecting), `on_disconnect` (error or close of the connected socket, and a
//! failed `Effect::Write`), `connect_failed` (the connect call itself threw),
//! `flush_timer_fired` and `reconnect_timer_fired` (one-shot timers started by
//! `ScheduleFlush` / `ScheduleReconnect`, not cancelled meanwhile).
//!
//! If an `Effect::Write` fails, call `on_disconnect` and drop the writes still
//! left in the same batch, as the TypeScript does (its second write finds the
//! socket gone).
use serde_json::{Value, json};

/// Activity changes are coalesced into one send per this many milliseconds.
pub const FLUSH_MS: u64 = 3000;
/// Re-probe this long after a drop or a failed probe.
pub const RECONNECT_MS: u64 = 5000;
/// Discord listens on `discord-ipc-0` to `discord-ipc-9`.
const LAST_PIPE: u32 = 9;

/// Counts shown on the profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Activity {
    pub downloading: u32,
    pub queued: u32,
}

/// What the app layer must do, in order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Effect {
    /// Open the pipe `pipe_path(i, ..)`; report `on_connect`, `on_probe_error(i)`
    /// or, if the call itself throws, `connect_failed`.
    Connect(u32),
    /// Write these bytes to the connected socket.
    Write(Vec<u8>),
    /// Destroy the probe socket that just failed.
    DestroyProbe,
    /// Destroy the connected socket.
    CloseSocket,
    /// Start a one-shot timer of `FLUSH_MS`.
    ScheduleFlush,
    CancelFlush,
    /// Start a one-shot timer of `RECONNECT_MS`.
    ScheduleReconnect,
    CancelReconnect,
}

/// The Discord IPC pipe to try. Windows has named pipes; elsewhere a socket
/// file under `$XDG_RUNTIME_DIR`, else `$TMPDIR`, else `/tmp` (an empty
/// variable counts as unset).
pub fn pipe_path(i: u32, windows: bool, xdg_runtime_dir: Option<&str>, tmpdir: Option<&str>) -> String {
    if windows {
        return format!(r"\\?\pipe\discord-ipc-{i}");
    }
    let base = [xdg_runtime_dir, tmpdir].into_iter().flatten().find(|s| !s.is_empty()).unwrap_or("/tmp");
    format!("{base}/discord-ipc-{i}")
}

/// A frame on the pipe: 4-byte little-endian opcode, 4-byte little-endian
/// payload length in bytes, then the JSON payload.
pub fn encode_frame(op: i32, data: &Value) -> Vec<u8> {
    let json = serde_json::to_vec(data).expect("a JSON value always serialises");
    let mut out = Vec::with_capacity(8 + json.len());
    out.extend_from_slice(&op.to_le_bytes());
    out.extend_from_slice(&(json.len() as i32).to_le_bytes());
    out.extend_from_slice(&json);
    out
}

#[derive(Debug, Default)]
pub struct Presence {
    pid: u32,
    client_id: String,
    connected: bool,
    pending: Option<Activity>,
    flush_scheduled: bool,
    reconnect_scheduled: bool,
    stopped: bool,
    nonce: u64,
}

impl Presence {
    /// `pid` is this process's id, sent with every activity.
    pub fn new(pid: u32) -> Self {
        Self { pid, ..Self::default() }
    }

    /// Begin advertising under the given application id. Empty stays silent.
    pub fn start(&mut self, client_id: &str) -> Vec<Effect> {
        if client_id.is_empty() {
            return vec![];
        }
        self.stopped = false;
        self.client_id = client_id.to_string();
        self.connect()
    }

    /// Queue an activity change; coalesced into one send per `FLUSH_MS`.
    pub fn set_activity(&mut self, activity: Activity) -> Vec<Effect> {
        if self.client_id.is_empty() {
            return vec![];
        }
        self.pending = Some(activity);
        if self.flush_scheduled {
            return vec![];
        }
        self.flush_scheduled = true;
        vec![Effect::ScheduleFlush]
    }

    pub fn stop(&mut self) -> Vec<Effect> {
        self.stopped = true;
        self.client_id.clear();
        self.pending = None;
        let mut effects = Vec::new();
        if std::mem::take(&mut self.flush_scheduled) {
            effects.push(Effect::CancelFlush);
        }
        if std::mem::take(&mut self.reconnect_scheduled) {
            effects.push(Effect::CancelReconnect);
        }
        effects.extend(self.destroy_socket());
        effects
    }

    pub fn on_connect(&mut self) -> Vec<Effect> {
        self.connected = true;
        // The handshake, then the current activity straight away.
        let mut effects = vec![Effect::Write(encode_frame(0, &json!({"v": 1, "client_id": self.client_id})))];
        effects.extend(self.flush());
        effects
    }

    pub fn on_probe_error(&mut self, i: u32) -> Vec<Effect> {
        let mut effects = vec![Effect::DestroyProbe];
        effects.extend(self.try_pipe(i + 1));
        effects
    }

    pub fn connect_failed(&mut self) -> Vec<Effect> {
        self.schedule_reconnect()
    }

    pub fn on_disconnect(&mut self) -> Vec<Effect> {
        let mut effects = self.destroy_socket();
        if !self.stopped && !self.client_id.is_empty() {
            effects.extend(self.schedule_reconnect());
        }
        effects
    }

    pub fn flush_timer_fired(&mut self) -> Vec<Effect> {
        self.flush_scheduled = false;
        self.flush()
    }

    pub fn reconnect_timer_fired(&mut self) -> Vec<Effect> {
        self.reconnect_scheduled = false;
        self.connect()
    }

    fn connect(&mut self) -> Vec<Effect> {
        if self.connected || self.client_id.is_empty() || self.stopped {
            return vec![];
        }
        self.try_pipe(0)
    }

    fn try_pipe(&mut self, i: u32) -> Vec<Effect> {
        if i > LAST_PIPE { self.schedule_reconnect() } else { vec![Effect::Connect(i)] }
    }

    fn flush(&mut self) -> Vec<Effect> {
        let Some(a) = self.pending.filter(|_| self.connected) else { return vec![] };
        let nonce = self.nonce.to_string();
        self.nonce += 1;
        let details = if a.downloading > 0 { format!("Downloading {}", a.downloading) } else { "Idle".to_string() };
        let frame = json!({
            "cmd": "SET_ACTIVITY",
            "args": { "pid": self.pid, "activity": { "details": details, "state": format!("{} queued", a.queued) } },
            "nonce": nonce,
        });
        vec![Effect::Write(encode_frame(1, &frame))]
    }

    fn schedule_reconnect(&mut self) -> Vec<Effect> {
        if self.reconnect_scheduled || self.stopped {
            return vec![];
        }
        self.reconnect_scheduled = true;
        vec![Effect::ScheduleReconnect]
    }

    fn destroy_socket(&mut self) -> Vec<Effect> {
        if std::mem::take(&mut self.connected) { vec![Effect::CloseSocket] } else { vec![] }
    }
}

/// One frame read from the pipe.
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    pub op: i32,
    /// `None` when the payload is not valid JSON.
    pub payload: Option<Value>,
}

/// Reassembles frames from bytes that arrive in arbitrary chunks. Not part of
/// electron/presence.ts, which never reads from the pipe: Discord's replies
/// (including a PING that wants a PONG) go unanswered there.
/// ponytail: a negative length (corrupt stream) drops what is buffered; there
/// is no upper bound on a frame, as the peer is the local Discord client.
#[derive(Debug, Default)]
pub struct FrameReader {
    buf: Vec<u8>,
}

impl FrameReader {
    pub fn feed(&mut self, bytes: &[u8]) -> Vec<Frame> {
        self.buf.extend_from_slice(bytes);
        let mut frames = Vec::new();
        let mut at = 0;
        while self.buf.len() - at >= 8 {
            let int = |i: usize| i32::from_le_bytes(self.buf[at + i..at + i + 4].try_into().expect("four bytes"));
            let (op, len) = (int(0), int(4));
            let Ok(len) = usize::try_from(len) else {
                at = self.buf.len();
                break;
            };
            if self.buf.len() - at < 8 + len {
                break;
            }
            let payload = serde_json::from_slice(&self.buf[at + 8..at + 8 + len]).ok();
            frames.push(Frame { op, payload });
            at += 8 + len;
        }
        self.buf.drain(..at);
        frames
    }
}
