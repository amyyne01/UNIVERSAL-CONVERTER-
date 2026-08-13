// HOW-THE-APP-WORKS §14: Discord rich presence. Advertises how many items are
// downloading / queued on the user's chat profile. Probes the local Discord IPC
// pipes until one answers, does the op-0 handshake, then keeps the status live —
// updates debounced/coalesced into occasional sends, reconnect scheduled on drop.
// Self-contained: clientId arrives via start(), activity via setActivity(); the
// only collaborator is node:net. Never throws when Discord is absent.
import net from 'node:net';

const FLUSH_MS = 3000; // coalesce activity changes into one send every few seconds
const RECONNECT_MS = 5000; // re-probe a few seconds after a drop

type Activity = { downloading: number; queued: number };

// Discord IPC frame: 4-byte LE op + 4-byte LE payload length + JSON.
function encode(op: number, data: unknown): Buffer {
  const json = Buffer.from(JSON.stringify(data));
  const header = Buffer.alloc(8);
  header.writeInt32LE(op, 0);
  header.writeInt32LE(json.length, 4);
  return Buffer.concat([header, json]);
}

// ponytail: app is Windows; the unix path is a courtesy fallback, not a tested path.
function pipePath(i: number): string {
  if (process.platform === 'win32') return `\\\\?\\pipe\\discord-ipc-${i}`;
  const base = process.env.XDG_RUNTIME_DIR || process.env.TMPDIR || '/tmp';
  return `${base}/discord-ipc-${i}`;
}

export class DiscordPresence {
  private clientId = '';
  private socket: net.Socket | null = null;
  private pending: Activity | null = null;
  private flushTimer: ReturnType<typeof setTimeout> | null = null;
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;
  private stopped = false;
  private nonce = 0;

  /** Begin advertising under the given app client id. Empty id → stay silent. */
  start(clientId: string): void {
    if (!clientId) return;
    this.stopped = false;
    this.clientId = clientId;
    this.connect();
  }

  /** Queue an activity change; coalesced into at most one send per FLUSH_MS. */
  setActivity(a: Activity): void {
    if (!this.clientId) return; // not configured → silent, no timers
    this.pending = a;
    if (this.flushTimer) return; // a send is already scheduled — coalesce into it
    this.flushTimer = later(() => {
      this.flushTimer = null;
      this.flush();
    }, FLUSH_MS);
  }

  stop(): void {
    this.stopped = true;
    this.clientId = '';
    this.pending = null;
    if (this.flushTimer) clearTimeout(this.flushTimer);
    if (this.reconnectTimer) clearTimeout(this.reconnectTimer);
    this.flushTimer = this.reconnectTimer = null;
    this.destroySocket();
  }

  // ── connection ────────────────────────────────────────────────────────────
  private connect(): void {
    if (this.socket || !this.clientId || this.stopped) return;
    this.tryPipe(0);
  }

  private tryPipe(i: number): void {
    if (i > 9) return void this.scheduleReconnect(); // no pipe answered
    let sock: net.Socket;
    try {
      sock = net.createConnection(pipePath(i));
    } catch {
      this.scheduleReconnect();
      return;
    }
    sock.once('connect', () => {
      sock.removeAllListeners('error');
      this.socket = sock;
      sock.on('error', () => this.onDisconnect());
      sock.on('close', () => this.onDisconnect());
      this.handshake();
    });
    sock.once('error', () => {
      try { sock.destroy(); } catch { /* already gone */ }
      this.tryPipe(i + 1); // step to the next candidate pipe
    });
  }

  private handshake(): void {
    this.write(0, { v: 1, client_id: this.clientId });
    this.flush(); // push the current activity right after connecting
  }

  private flush(): void {
    if (!this.pending || !this.socket) return;
    this.write(1, {
      cmd: 'SET_ACTIVITY',
      args: {
        pid: process.pid,
        activity: {
          details: this.pending.downloading > 0 ? `Downloading ${this.pending.downloading}` : 'Idle',
          state: `${this.pending.queued} queued`,
        },
      },
      nonce: String(this.nonce++),
    });
  }

  private write(op: number, data: unknown): void {
    try {
      this.socket?.write(encode(op, data));
    } catch {
      this.onDisconnect();
    }
  }

  private onDisconnect(): void {
    this.destroySocket();
    if (this.stopped || !this.clientId) return;
    this.scheduleReconnect();
  }

  private scheduleReconnect(): void {
    if (this.reconnectTimer || this.stopped) return;
    this.reconnectTimer = later(() => {
      this.reconnectTimer = null;
      this.connect();
    }, RECONNECT_MS);
  }

  private destroySocket(): void {
    if (!this.socket) return;
    const s = this.socket;
    this.socket = null;
    try { s.destroy(); } catch { /* already gone */ }
  }
}

/** setTimeout that won't keep the process alive on its own (Electron's loop stays up). */
function later(fn: () => void, ms: number): ReturnType<typeof setTimeout> {
  const t = setTimeout(fn, ms);
  t.unref?.();
  return t;
}
