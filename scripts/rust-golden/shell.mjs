// Golden file for the thin shell around the engine: the ahg:// protocol links
// (electron/protocol.ts), the clipboard link watcher (clipboard-watch.ts),
// Discord presence (presence.ts), and the decision parts of the download
// feedback, global hotkey and tray modules. Every Electron and Node
// collaborator is replaced by a recording fake and put back afterwards.
import { register } from 'node:module';
import net from 'node:net';
import path from 'node:path';
import { EventEmitter } from 'node:events';
import { writeFileSync } from 'node:fs';

/** A number JSON cannot carry is written as its name. */
const num = (x) => (Number.isFinite(x) ? x : String(x));

export default async function shell({ OUT, literals }) {
  // The main script's 'electron' stand-in only knows app.getPath. This one is
  // registered after it, so it answers first, and hands every call to whatever
  // is in globalThis.__fakes at that moment.
  register(`data:text/javascript,${encodeURIComponent(`
    const STUB = 'data:text/javascript,' + encodeURIComponent(
      'const f = () => globalThis.__fakes;' +
      'export const app = { getPath: (n) => process.env["AHG_GOLDEN_" + n.toUpperCase()], setAsDefaultProtocolClient: (...a) => f().app.setAsDefaultProtocolClient(...a) };' +
      'export const clipboard = { readText: (...a) => f().clipboard.readText(...a) };' +
      'export const globalShortcut = { register: (...a) => f().globalShortcut.register(...a), unregister: (...a) => f().globalShortcut.unregister(...a), unregisterAll: (...a) => f().globalShortcut.unregisterAll(...a) };' +
      'export class Tray { constructor(...a) { return f().Tray(...a); } }' +
      'export const Menu = { buildFromTemplate: (...a) => f().Menu.buildFromTemplate(...a) };' +
      'export const nativeImage = { createFromPath: (...a) => f().nativeImage.createFromPath(...a) };' +
      'export const session = {}; export default {};' +
      '/* shell golden */');
    export async function resolve(specifier, context, next) {
      return specifier === 'electron' ? { url: STUB, shortCircuit: true } : next(specifier, context);
    }
  `)}`);

  const real = {
    setInterval: globalThis.setInterval, clearInterval: globalThis.clearInterval,
    setTimeout: globalThis.setTimeout, clearTimeout: globalThis.clearTimeout,
    consoleError: console.error, createConnection: net.createConnection,
    platform: Object.getOwnPropertyDescriptor(process, 'platform'), argv: process.argv,
    pid: Object.getOwnPropertyDescriptor(process, 'pid'),
    defaultApp: Object.getOwnPropertyDescriptor(process, 'defaultApp'),
    xdg: process.env.XDG_RUNTIME_DIR, tmp: process.env.TMPDIR,
  };
  const out = {};
  try {
    out.protocol = await protocolSection(literals);
    out.clipboard = await clipboardSection(literals);
    out.presence = await presenceSection(literals);
    Object.assign(out, await feedbackSection(), await hotkeySection(), await traySection());
  } finally {
    Object.assign(globalThis, {
      setInterval: real.setInterval, clearInterval: real.clearInterval,
      setTimeout: real.setTimeout, clearTimeout: real.clearTimeout,
    });
    console.error = real.consoleError;
    net.createConnection = real.createConnection;
    Object.defineProperty(process, 'platform', real.platform);
    Object.defineProperty(process, 'pid', real.pid);
    process.argv = real.argv;
    if (real.defaultApp) Object.defineProperty(process, 'defaultApp', real.defaultApp); else delete process.defaultApp;
    for (const [k, v] of [['XDG_RUNTIME_DIR', real.xdg], ['TMPDIR', real.tmp]]) {
      if (v === undefined) delete process.env[k]; else process.env[k] = v;
    }
    delete globalThis.__fakes;
  }
  writeFileSync(`${OUT}/shell.json`, JSON.stringify(out) + '\n');
  console.log(`shell.json: protocol ${out.protocol.extract.length + out.protocol.resolve.length + out.protocol.registration.length}, `
    + `clipboard ${out.clipboard.scenarios.length}, presence ${out.presence.scenarios.length + out.presence.decode.length}, `
    + `feedback ${out.feedback.length}, hotkey ${out.hotkey.length}, tray ${out.tray.cases.length}`);
}

// ── protocol.ts ──────────────────────────────────────────────────────────────
async function protocolSection(literals) {
  const calls = [];
  globalThis.__fakes = { app: { setAsDefaultProtocolClient: (...a) => calls.push(a) } };
  const p = await import('../../dist-electron/electron/protocol.js');

  const YT = 'https://youtu.be/dQw4w9WgXcQ';
  const pad = (head, total, fill = 'x') => head + fill.repeat(total - head.length);
  const payloads = [
    ...literals('electron/protocol.test.ts').map((l) => l.replace(/^ahg:\/\//, '')),
    YT, 'https://youtube.com/watch?v=dQw4w9WgXcQ', 'HTTPS://youtu.be/dQw4w9WgXcQ', 'Http://youtu.be/dQw4w9WgXcQ',
    'http://youtu.be/dQw4w9WgXcQ', 'https://www.youtube.com/playlist?list=PL123', 'https://open.spotify.com/track/4cOdK2wGLETKBW3PvgPWqT',
    'https://soundcloud.com/a/b', 'https://www.instagram.com/reel/Cx1/', 'https://www.tiktok.com/@u/video/123', 'https://vimeo.com/123456',
    'https://example.com/page', 'https://example.com/a.mp4', 'https://cdn.example.com/a.MP4?x=1', 'https://localhost/x',
    'ytsearch:artist - title', 'ytsearch:watch youtube.com/watch?v=dQw4w9WgXcQ', 'ytsearch:some song.mp4', 'ytsearch:some song.mp4 ',
    'YTSEARCH:x.mp4', 'YTSEARCH:artist', 'ytsearch5:x.mp4', 'ytsearch:', 'ytsearch: ' + YT,
    'https%3A%2F%2Fyoutu.be%2FdQw4w9WgXcQ', 'https://youtu.be/%64Qw4w9WgXcQ', 'https://youtu.be/dQw4w9WgXcQ%0A', 'https://youtu.be/dQw4w9WgXcQ%',
    'ahg://' + YT, 'ahg://ahg://' + YT,
    '', ' ', ' ' + YT, YT + ' ', '\t' + YT, YT + '\n', YT + '\r', YT + '\r\n', 'https://youtu.be/\ndQw4w9WgXcQ', YT + '\u2028', YT + '\u0085',
    '-', '-exec=calc', '--version', '-' + YT, 'ftp://example.com/a.mp4', 'file:///c:/a.mp4', 'javascript:alert(1)', 'spotify:track:abc',
    'mailto:a@b.c', 'data:text/html,hi', 'youtu.be/dQw4w9WgXcQ', '//youtu.be/dQw4w9WgXcQ', 'https:/youtu.be/dQw4w9WgXcQ', 'https:youtu.be/dQw4w9WgXcQ',
    pad('https://youtu.be/', 2048), pad('https://youtu.be/', 2049), pad('https://example.com/', 2048) + '.mp4',
    'https://youtu.be/' + '\uD83D\uDE00'.repeat(1015),
    'https://youtu.be/' + '\uD83D\uDE00'.repeat(1016), 'https://youtu.be/' + '\uD83D\uDE00'.repeat(1015) + 'x',
    'https://youtu.be/' + '\u00E9'.repeat(2031), 'https://youtu.be/' + '\u00E9'.repeat(2032), 'a'.repeat(3000),
    'https://b\u00FCcher.example/x.mp4', 'https://\u202Eevil.example/x.mp4', 'https://exa mple.com/x.mp4',
  ];
  const links = [...new Set([
    ...payloads.map((x) => 'ahg://' + x),
    'ahg:https://youtu.be/dQw4w9WgXcQ', 'ahg:/https://youtu.be/dQw4w9WgXcQ', 'ahg//https://youtu.be/dQw4w9WgXcQ', 'ahg:' + YT, 'ahg' + YT,
    'AHG://' + YT, 'Ahg://' + YT, 'aHg://' + YT, ' ahg://' + YT, '\uFEFFahg://' + YT, 'xahg://' + YT, 'ahg://' + 'https://youtu.be/dQw4w9WgXcQ'.toUpperCase(),
    'ahg://', 'ahg:///', 'ahg:////', 'ahg://x', 'ahg', 'ahg:', 'a', '', ' ', YT, 'https://youtube.com/watch?v=abc12345678',
    'ahg-dev://' + YT, 'ahg+x://' + YT, 'ahg://\u0000' + YT,
  ])];
  const resolve = links.map((raw) => ({ raw, out: p.resolveProtocolLink(raw) }));

  const L = 'ahg://' + YT;
  const L2 = 'ahg://https://youtube.com/watch?v=abc12345678';
  const argvs = [
    [], [''], ['C:\\app.exe'], ['C:\\app.exe', '--some-flag'], ['C:\\app.exe', L], [L, 'C:\\app.exe'], ['C:\\app.exe', '--x', L],
    ['C:\\app.exe', L, L2], ['C:\\app.exe', L2, L, L], [L, L], ['C:\\app.exe', 'ahg://'], ['C:\\app.exe', 'ahg://', L], ['C:\\app.exe', 'AHG://x', L],
    ['C:\\app.exe', ' ' + L, L2], ['C:\\app.exe', 'xahg://y', 'ahg:/y', 'ahg:y'], ['C:\\app.exe', '--url=' + L], ['C:\\app.exe', 'ahg://x.com'],
    ['C:\\app.exe', '\u00E9ahg://x'], ['C:\\app.exe', 'ahg://\u00E9'], ['ahg://-exec=calc', L],
    ...literals('electron/protocol.test.ts').filter((l) => !l.startsWith('ahg://') || true).map((l) => ['C:\\app.exe', l]),
  ];
  const extract = argvs.map((argv) => ({ argv, out: p.extractProtocolUrl(argv) }));

  // registerProtocolHandler: which registration the OS gets.
  const registration = [];
  for (const defaultApp of [false, true]) {
    for (const argv of [[], ['C:\\electron.exe'], ['C:\\electron.exe', 'C:\\app\\main.js'], ['C:\\electron.exe', 'main.js', 'x']]) {
      calls.length = 0;
      Object.defineProperty(process, 'defaultApp', { value: defaultApp, configurable: true, writable: true });
      process.argv = argv;
      p.registerProtocolHandler();
      const [call] = calls;
      registration.push({
        defaultApp, argvLen: argv.length,
        call: calls.length === 0 ? 'none' : call.length === 1 ? 'plain' : 'script',
        scheme: call?.[0] ?? null,
        // The script path the OS is given must be argv[1] made absolute.
        scriptOk: call?.length === 3 ? call[2][0] === path.resolve(argv[1]) && call[1] === process.execPath : null,
      });
    }
  }
  return { extract, resolve, registration };
}

// ── clipboard-watch.ts ───────────────────────────────────────────────────────
async function clipboardSection(literals) {
  globalThis.__fakes = { clipboard: { readText: () => { throw new Error('the real clipboard must not be read'); } } };
  const { ClipboardWatcher } = await import('../../dist-electron/electron/clipboard-watch.js');

  const timers = [];
  globalThis.setInterval = (fn, ms) => { const t = { fn, ms, cleared: false, unref() {} }; timers.push(t); return t; };
  globalThis.clearInterval = (t) => { t.cleared = true; };
  let errors = 0;
  console.error = () => { errors++; };

  let pollMs = null;
  const run = (steps) => {
    timers.length = 0;
    let clip = '';
    let reads = 0;
    let enabled = true;
    const offers = [];
    const w = new ClipboardWatcher({
      isEnabled: () => enabled,
      onDetect: (d) => offers.push(JSON.parse(JSON.stringify(d))),
      readClipboard: () => { reads++; if (clip === null) throw new Error('clipboard unavailable'); return clip; },
    });
    const live = () => timers.filter((t) => !t.cleared).at(-1);
    return steps.map((step) => {
      reads = 0; errors = 0; offers.length = 0;
      let ran = true;
      let ms = null;
      if (step.op === 'start') {
        clip = step.clip;
        const before = timers.length;
        w.start();
        ms = timers.length > before ? timers.at(-1).ms : null;
      } else if (step.op === 'stop') {
        w.stop();
      } else {
        clip = step.clip;
        enabled = step.enabled;
        const t = live();
        if (t) t.fn(); else ran = false;
      }
      if (ms) pollMs = ms;
      return { ...step, ran, ms, reads, errors, offer: offers[0] ?? null, offers: offers.length };
    });
  };

  const start = (clip = '') => ({ op: 'start', clip });
  const tick = (clip, enabled = true) => ({ op: 'tick', clip, enabled });
  const YT = 'https://youtu.be/dQw4w9WgXcQ';
  const texts = [...new Set([
    ...literals('electron/clipboard-watch.test.ts'),
    YT, 'https://youtu.be/anotherID1', ' ' + YT + ' ', '\n\t' + YT + '\r\n', YT + '\u00A0', '\uFEFF' + YT, '\u200B' + YT, YT + '\u2028', YT + '\u0085',
    'https://example.com', 'https://example.com/a.mp4', 'http://localhost/x', 'example.com', 'ytsearch:foo', 'ytsearch:foo.mp4', 'ahg://' + YT,
    'https://open.spotify.com/track/4cOdK2wGLETKBW3PvgPWqT', 'https://www.tiktok.com/@u/video/123', 'https://vimeo.com/123456',
    'https://soundcloud.com/a/b', 'https://www.youtube.com/playlist?list=PL123', 'check this out ' + YT, YT + ' and more words',
    '', ' ', '\n', '\u00A0', '\uFEFF', 'just some copied text', 'x'.repeat(100000), 'x'.repeat(100000) + ' ' + YT,
    'https://youtu.be/' + 'x'.repeat(5000), YT + '\n' + YT, '\uD83D\uDE00 ' + YT,
  ])];
  const scenarios = [
    // The five sequences of the test file, then every text through the same shape.
    [start(), tick(YT), tick(YT), tick(YT)],
    [start(YT), tick(YT), tick(YT)],
    [start(YT), tick('other'), tick(YT)],
    [start(), tick(YT, false), tick(YT, false), tick('https://youtu.be/anotherID1', true), tick('https://youtu.be/anotherID1', true)],
    [start(), tick(YT), tick('something else'), tick(YT)],
    [start(null), tick(YT), tick(null), tick(YT)],
    [start(), tick(null), tick(YT)],
    [start(' ' + YT + ' '), tick(YT), tick('\n' + YT)],
    [start(YT), start('https://youtu.be/anotherID1'), tick('https://youtu.be/anotherID1'), tick(YT)],
    [start(), stop(), tick(YT), start(YT), tick('https://youtu.be/anotherID1'), stop(), stop(), start('other'), tick(YT)],
    [start(YT), stop(), start(null), tick(YT), tick('https://youtu.be/anotherID1')],
    [tick(YT), start(), tick(YT)],
    [start(), tick('   '), tick(YT), tick('   '), tick(YT), tick('')],
    [start(), tick('https://example.com'), tick('https://example.com'), tick('https://example.com/a.mp4'), tick('just text'), tick('https://example.com')],
    ...texts.map((t) => [start(), tick(t), tick(t), tick('other text'), tick(t), tick(t, false), tick('zzz', false), tick(t)]),
    ...texts.map((t) => [start(t), tick(t), tick(t.trim()), tick(' ' + t + ' ')]),
    ...texts.slice(0, 12).map((t, i) => [start(), ...texts.slice(i, i + 6).map((x, j) => tick(x, j % 3 !== 2))]),
  ];
  function stop() { return { op: 'stop' }; }
  const results = scenarios.map(run);
  return { pollMs, scenarios: results };
}

// ── presence.ts ──────────────────────────────────────────────────────────────
async function presenceSection(literals) {
  globalThis.__fakes = {};
  // A fixed process id keeps the golden file the same from run to run.
  const pid = 4242;
  Object.defineProperty(process, 'pid', { value: pid, configurable: true });
  const { DiscordPresence } = await import('../../dist-electron/electron/presence.js');

  let events = [];
  let sockets = [];
  let timers = [];
  let failNextConnect = false;
  let failNextWrite = false;
  net.createConnection = (p) => {
    events.push({ op: 'connect', path: p });
    if (failNextConnect) { failNextConnect = false; throw new Error('connect failed'); }
    const sock = new EventEmitter();
    sock.pipe = Number(/discord-ipc-(\d+)$/.exec(p)[1]);
    sock.write = (buf) => {
      events.push({ op: 'write', hex: Buffer.from(buf).toString('hex') });
      if (failNextWrite) { failNextWrite = false; throw new Error('write failed'); }
    };
    sock.destroy = () => { events.push({ op: 'destroy' }); };
    sockets.push(sock);
    return sock;
  };
  globalThis.setTimeout = (fn, ms) => {
    const t = { fn, ms, active: true, unref() {} };
    timers.push(t);
    events.push({ op: 'set', ms });
    return t;
  };
  globalThis.clearTimeout = (t) => { events.push({ op: 'clear', ms: t.ms }); t.active = false; };

  const run = ({ platform, xdg, tmp }, ops) => {
    events = []; sockets = []; timers = []; failNextConnect = failNextWrite = false;
    Object.defineProperty(process, 'platform', { value: platform, configurable: true });
    for (const [k, v] of [['XDG_RUNTIME_DIR', xdg], ['TMPDIR', tmp]]) {
      if (v === null) delete process.env[k]; else process.env[k] = v;
    }
    const p = new DiscordPresence();
    return ops.map((op) => {
      events = [];
      const rec = { ...op };
      if (op.op === 'start') p.start(op.id);
      else if (op.op === 'activity') p.setActivity({ downloading: op.downloading, queued: op.queued });
      else if (op.op === 'stop') p.stop();
      else if (op.op === 'failNextConnect') failNextConnect = true;
      else if (op.op === 'failNextWrite') failNextWrite = true;
      else if (op.op === 'fire') {
        const t = timers.filter((x) => x.active && x.ms === op.ms).at(-1);
        rec.fired = !!t;
        if (t) { t.active = false; t.fn(); }
      } else if (op.op === 'emit') {
        const sock = sockets[op.sock];
        rec.pipe = sock.pipe;
        rec.handlers = { connect: sock.listenerCount('connect'), error: sock.listenerCount('error'), close: sock.listenerCount('close') };
        if (op.ev === 'error' && rec.handlers.error === 0) throw new Error('corpus emits an error nobody listens to');
        sock.emit(op.ev);
      }
      rec.events = events;
      return rec;
    });
  };

  const start = (id = 'client-1') => ({ op: 'start', id });
  const act = (downloading, queued) => ({ op: 'activity', downloading, queued });
  const flush = () => ({ op: 'fire', ms: 3000 });
  const reconn = () => ({ op: 'fire', ms: 5000 });
  const emit = (sock, ev) => ({ op: 'emit', sock, ev });
  const stop = () => ({ op: 'stop' });
  const failConnect = () => ({ op: 'failNextConnect' });
  const failWrite = () => ({ op: 'failNextWrite' });
  const win = { platform: 'win32', xdg: null, tmp: null };
  const envs = [
    win, { platform: 'win32', xdg: '/x', tmp: '/y' },
    { platform: 'linux', xdg: '/run/user/1000', tmp: '/tmpx' }, { platform: 'linux', xdg: '', tmp: '/tmpx' },
    { platform: 'linux', xdg: null, tmp: '/tmpx' }, { platform: 'linux', xdg: '', tmp: '' }, { platform: 'linux', xdg: null, tmp: null },
    { platform: 'darwin', xdg: '/xdg', tmp: null },
  ];
  const ids = [...new Set([
    ...literals('electron/presence.test.ts').filter((l) => /^[\w-]+$/.test(l)),
    'cid', 'app-123', '123456789012345678', 'a"b\\c', 'line\nbreak\ttab\u0001\u001F\u007F', '\u00E9\u00E8 \u4E2D\u6587 \uD83D\uDE00', '\u2028\u2029',
    '</script>', 'x'.repeat(5000), '\uD83D\uDE00'.repeat(3000), ' ',
  ])];
  const counts = [0, 1, 2, 7, 8, 9, 10, 99, 100, 12345, 4294967295];

  const sweep = [...Array(10).keys()].map((i) => emit(i, 'error'));
  const flows = [
    // connect, handshake, debounced update
    [start(), emit(0, 'connect'), act(1, 2), act(3, 4), flush(), flush(), act(0, 0), flush()],
    [start(), act(2, 5), act(0, 1), emit(0, 'connect'), flush(), act(4, 0), flush(), act(4, 0), flush()],
    [act(1, 1), flush(), start(), emit(0, 'connect'), act(1, 1), flush()],
    [start(''), act(1, 1), flush()],
    // every pipe refuses: step through all ten, then wait and probe again
    [start(), ...sweep, reconn(), ...[...Array(10).keys()].map((i) => emit(i + 10, 'error')), reconn()],
    [start(), emit(0, 'error'), emit(1, 'error'), emit(2, 'connect'), act(1, 0), flush()],
    // connection drops: error, close, both, repeatedly
    [start(), emit(0, 'connect'), emit(0, 'error'), emit(0, 'close'), reconn(), emit(1, 'connect'), act(1, 1), flush(), emit(1, 'close'), reconn(), emit(2, 'error')],
    [start(), emit(0, 'connect'), emit(0, 'close'), emit(0, 'close'), reconn(), reconn(), emit(1, 'error'), emit(2, 'connect'), flush()],
    [start(), emit(0, 'connect'), act(1, 1), emit(0, 'close'), flush(), reconn(), emit(1, 'connect'), flush(), act(2, 2), flush()],
    // throwing collaborators
    [failConnect(), start(), reconn(), emit(0, 'connect'), act(1, 0), flush()],
    [start(), failConnect(), emit(0, 'error'), reconn(), emit(1, 'error'), emit(2, 'connect')],
    [start(), failWrite(), emit(0, 'connect'), reconn(), emit(1, 'connect'), act(1, 1), failWrite(), flush(), reconn(), emit(2, 'connect'), flush()],
    [act(1, 1), start(), failWrite(), emit(0, 'connect'), reconn(), emit(1, 'connect'), flush()],
    // stop in every state
    [start(), stop(), emit(0, 'connect'), act(1, 1), flush(), reconn()],
    [start(), emit(0, 'connect'), act(1, 1), stop(), flush(), emit(0, 'close'), reconn()],
    [start(), emit(0, 'connect'), act(1, 1), flush(), stop(), start('second'), emit(1, 'connect'), act(2, 2), flush()],
    [start(), emit(0, 'connect'), emit(0, 'close'), stop(), reconn(), act(1, 1), flush()],
    [start(), ...sweep, stop(), reconn()],
    [start(), act(1, 1), act(2, 2), stop(), flush(), act(3, 3), flush()],
    [stop(), stop(), start(), stop(), stop()],
    // starting twice, and a second probe racing the first
    [start(), start(), emit(0, 'connect'), emit(1, 'connect'), act(1, 1), flush()],
    [start(), emit(0, 'connect'), start('other'), act(1, 1), flush()],
    [start(), emit(0, 'error'), start('other'), emit(1, 'error'), emit(2, 'connect')],
    [start(), emit(0, 'error'), emit(1, 'error'), reconn(), reconn(), emit(2, 'connect')],
    // nonce keeps counting across reconnects
    [start(), emit(0, 'connect'), act(1, 1), flush(), act(1, 2), flush(), emit(0, 'close'), reconn(), emit(1, 'connect'), act(1, 3), flush(), flush()],
  ];
  const scenarios = [];
  for (const env of envs) for (const ops of flows) scenarios.push({ env, ops: run(env, ops) });
  // Every activity shape through one live connection.
  for (const d of counts) for (const q of counts) scenarios.push({ env: win, ops: run(win, [start(), emit(0, 'connect'), act(d, q), flush()]) });
  // Every client id through the handshake.
  for (const id of ids) scenarios.push({ env: win, ops: run(win, [start(id), emit(0, 'connect'), act(1, 1), flush()]) });

  // Decoding what Discord sends back. The app never reads the pipe, so there is
  // no TypeScript to run: this is a reference reader over Buffer, shaped like the
  // decode() helper in presence.test.ts, that the Rust reader has to agree with.
  const frame = (op, json) => {
    const body = typeof json === 'string' ? Buffer.from(json) : Buffer.from(JSON.stringify(json));
    const head = Buffer.alloc(8);
    head.writeInt32LE(op, 0);
    head.writeInt32LE(body.length, 4);
    return Buffer.concat([head, body]);
  };
  const reference = (chunks) => {
    let buf = Buffer.alloc(0);
    return chunks.map((chunk) => {
      buf = Buffer.concat([buf, chunk]);
      const frames = [];
      while (buf.length >= 8) {
        const len = buf.readInt32LE(4);
        if (len < 0) { buf = Buffer.alloc(0); break; }
        if (buf.length < 8 + len) break;
        let payload = null;
        try { payload = JSON.parse(buf.subarray(8, 8 + len).toString()); } catch { /* not JSON */ }
        frames.push({ op: buf.readInt32LE(0), payload });
        buf = buf.subarray(8 + len);
      }
      return frames;
    });
  };
  const ready = frame(1, { cmd: 'DISPATCH', evt: 'READY', data: { v: 1, user: { id: '1', username: 'u\u00E9' } }, nonce: null });
  const ack = frame(1, { cmd: 'SET_ACTIVITY', data: { name: 'x' }, evt: null, nonce: '0' });
  const err = frame(1, { cmd: 'SET_ACTIVITY', evt: 'ERROR', data: { code: 4000, message: 'Invalid Client ID' } });
  const closeF = frame(2, { code: 4000, message: 'Invalid Client ID' });
  const ping = frame(3, { n: 1 });
  const pong = frame(4, {});
  const unicode = frame(1, { text: '\u4E2D\u6587\uD83D\uDE00' });
  const cuts = (b) => [...Array(b.length).keys()].map((i) => [b.subarray(0, i + 1)]);
  const bytewise = (b) => [...b].map((x) => Buffer.from([x]));
  const decodeInputs = [
    [ready], [ack], [err], [closeF], [ping], [pong], [unicode], [frame(0, { v: 1, client_id: 'x' })], [frame(1, {})], [frame(7, [1, 2])],
    [Buffer.alloc(0)], [Buffer.alloc(3)], [Buffer.alloc(8)], [frame(1, '')], [frame(1, 'not json')], [frame(1, '{"a":')], [frame(2, 'null')], [frame(1, '100')],
    [Buffer.concat([ready, ack])], [Buffer.concat([ping, pong, closeF])], [Buffer.concat([ready, ack.subarray(0, 5)]), ack.subarray(5)],
    [ready.subarray(0, 4), ready.subarray(4, 8), ready.subarray(8)], [ready.subarray(0, 7), ready.subarray(7, 9), ready.subarray(9)],
    [ready.subarray(0, 8), Buffer.alloc(0), ready.subarray(8)], bytewise(ready), bytewise(unicode), bytewise(Buffer.concat([ping, pong, err])),
    [unicode.subarray(0, 8 + 14), unicode.subarray(8 + 14)], [unicode.subarray(0, unicode.length - 1), unicode.subarray(unicode.length - 1)],
    [frame(1, { big: 'y'.repeat(70000) })], [frame(1, { big: 'y'.repeat(70000) }).subarray(0, 40000), frame(1, { big: 'y'.repeat(70000) }).subarray(40000)],
    [Buffer.from([1, 0, 0, 0, 0xff, 0xff, 0xff, 0xff, 1, 2, 3]), ready], [Buffer.from([1, 0, 0, 0, 0xff, 0xff, 0xff, 0xff]), Buffer.alloc(0)],
    [Buffer.from([0xff, 0xff, 0xff, 0xff, 2, 0, 0, 0, 0x7b, 0x7d])], [Buffer.from([0, 0, 0, 0x80, 2, 0, 0, 0, 0x7b, 0x7d])],
    ...cuts(ping).map((c) => [...c, ping.subarray(c[0].length)]),
  ].map((chunks) => ({ chunks: chunks.map((c) => Buffer.from(c).toString('hex')), out: reference(chunks.map((c) => Buffer.from(c))) }));

  return { flushMs: 3000, reconnectMs: 5000, pid, scenarios, decode: decodeInputs };
}

// ── feedback.ts ──────────────────────────────────────────────────────────────
async function feedbackSection() {
  const { downloadFeedback } = await import('../../dist-electron/electron/feedback.js');
  const statuses = ['queued', 'fetching_info', 'downloading', 'converting', 'embedding', 'paused', 'retrying', 'failed', 'done', 'cancelled'];
  const t = (status, percent = 0, title = 'Song') => ({ status, percent, title });
  const percents = [0, 1, 20, 33.3, 50, 60, 99.9, 100, 0.1, 0.2, 0.3, 1e-7, 150, -5, NaN, Infinity, -Infinity];
  const sequences = [
    // test file
    [{ op: 'progress', tasks: [t('downloading', 20, 'a'), t('converting', 60, 'b'), t('done', 100, 'c'), t('queued', 0, 'd')] }, { op: 'progress', tasks: [t('done', 100, 'a')] }],
    [{ op: 'progress', tasks: [] }, { op: 'progress', tasks: [] }],
    [{ op: 'completed', ...t('done', 100, 'My Song'), enabled: true }, { op: 'completed', ...t('done', 100), enabled: false }],
    [{ op: 'completed', ...t('cancelled'), enabled: true }, { op: 'completed', ...t('failed'), enabled: true }],
    // titles
    ...['', 'x', ' ', '0', 'Title \u00E9 \uD83D\uDE00', 'a'.repeat(5000), 'line\nbreak'].map((title) => [{ op: 'completed', ...t('done', 100, title), enabled: true }]),
    // every status, both settings
    ...statuses.map((s) => [{ op: 'completed', ...t(s, 100, 'T'), enabled: true }, { op: 'completed', ...t(s, 100, 'T'), enabled: false }]),
    // the bar, status by status and percent by percent
    ...statuses.map((s) => [{ op: 'progress', tasks: [t(s, 50)] }, { op: 'progress', tasks: [t(s, 50)] }, { op: 'progress', tasks: [] }, { op: 'progress', tasks: [t(s, 75)] }]),
    ...percents.map((p) => [{ op: 'progress', tasks: [t('downloading', p)] }, { op: 'progress', tasks: [t('downloading', p)] }, { op: 'progress', tasks: [t('converting', p), t('embedding', 10)] }]),
    // averaging, floating point and ordering
    [{ op: 'progress', tasks: [t('downloading', 0.1), t('downloading', 0.2), t('downloading', 0.3)] }],
    [{ op: 'progress', tasks: [t('downloading', 0.3), t('downloading', 0.2), t('downloading', 0.1)] }],
    [{ op: 'progress', tasks: [t('downloading', 33.3), t('converting', 66.6), t('embedding', 99.9), t('fetching_info', 0)] }],
    [{ op: 'progress', tasks: [...Array(7)].map((_, i) => t('downloading', i * 14.3)) }],
    [{ op: 'progress', tasks: [t('downloading', 1e308), t('downloading', 1e308)] }],
    [{ op: 'progress', tasks: [t('downloading', 10)] }, { op: 'progress', tasks: [t('downloading', 10), t('queued', 90)] }, { op: 'progress', tasks: [t('downloading', 10), t('paused', 90)] }, { op: 'progress', tasks: [] }, { op: 'progress', tasks: [] }],
    [{ op: 'progress', tasks: [t('downloading', NaN)] }, { op: 'progress', tasks: [t('downloading', NaN)] }, { op: 'progress', tasks: [] }, { op: 'progress', tasks: [] }],
    [{ op: 'progress', tasks: [t('downloading', 40)] }, { op: 'progress', tasks: [t('downloading', 20), t('downloading', 60)] }, { op: 'progress', tasks: [t('downloading', 40)] }],
    [{ op: 'progress', tasks: [t('downloading', 100)] }, { op: 'completed', ...t('done', 100), enabled: true }, { op: 'progress', tasks: [t('done', 100)] }],
  ];
  const cases = sequences.map((steps) => {
    const notified = [];
    const progress = [];
    const fb = downloadFeedback({
      enabled: () => cur.enabled,
      notify: (b) => notified.push(b),
      setProgress: (p) => progress.push(num(p)),
    });
    let cur = {};
    return steps.map((step) => {
      notified.length = 0; progress.length = 0;
      cur = step;
      const task = { taskId: step.title, title: step.title, progress: { status: step.status, percent: step.percent } };
      if (step.op === 'completed') fb.completed(task);
      else fb.progress(step.tasks.map((x) => ({ taskId: x.title, title: x.title, progress: { status: x.status, percent: x.percent } })));
      return {
        op: step.op, status: step.status ?? null, title: step.title ?? null, enabled: step.enabled ?? null,
        tasks: step.tasks?.map((x) => ({ status: x.status, percent: num(x.percent) })) ?? null,
        notified: [...notified], progress: [...progress],
      };
    });
  });
  return { feedback: cases };
}

// ── hotkey.ts ────────────────────────────────────────────────────────────────
async function hotkeySection() {
  const calls = [];
  let ret;
  globalThis.__fakes = {
    globalShortcut: {
      register: (...a) => { calls.push(['register', a[0], typeof a[1]]); return ret; },
      unregister: (...a) => { calls.push(['unregister', a[0]]); },
      unregisterAll: () => { calls.push(['unregisterAll']); },
    },
  };
  const h = await import('../../dist-electron/electron/hotkey.js');
  const accelerators = [
    'CommandOrControl+Shift+D', 'Ctrl+Alt+V', '', ' ', '  CommandOrControl+Shift+D ', 'ctrl+shift+d', 'Shift', 'Ctrl+', '+', 'F13', 'Super+X',
    'Ctrl+Shift+\u00E9', 'a'.repeat(1000), 'not an accelerator', 'Ctrl+Shift+D\n', '0',
  ];
  const cases = [];
  for (const accel of accelerators) {
    for (const r of ['true', 'false', 'undefined', 'null']) {
      ret = { true: true, false: false, undefined: undefined, null: null }[r];
      calls.length = 0;
      const result = h.registerGlobalHotkey(accel, () => {});
      const registerCalls = calls.map((c) => c.join(':'));
      calls.length = 0;
      h.unregisterGlobalHotkey(accel);
      cases.push({ accelerator: accel, electronReturns: r, result, registerCalls, unregisterCalls: calls.map((c) => c.join(':')) });
    }
  }
  calls.length = 0;
  h.unregisterAllHotkeys();
  return { hotkey: cases, hotkeyAll: calls.map((c) => c.join(':')) };
}

// ── tray.ts ──────────────────────────────────────────────────────────────────
async function traySection() {
  let template;
  const tooltips = [];
  let trayArg;
  let destroyed = 0;
  globalThis.__fakes = {
    Tray: (img) => { trayArg = img; return { setToolTip: (t) => tooltips.push(t), setContextMenu: (m) => { template = m; }, destroy: () => { destroyed++; } }; },
    Menu: { buildFromTemplate: (t) => t },
    nativeImage: { createFromPath: (p) => ({ path: p }) },
  };
  const { initTray } = await import('../../dist-electron/electron/tray.js');
  const statuses = ['queued', 'fetching_info', 'downloading', 'converting', 'embedding', 'paused', 'retrying', 'failed', 'done', 'cancelled'];
  const names = ['pauseAll', 'resumeAll', 'toggleWindow', 'pasteAndDownload', 'quit'];
  const actions = Object.fromEntries(names.map((n) => [n, () => {}]));
  const titles = ['Song A', '', ' ', 'a'.repeat(3000), 'two\nlines', 'Caf\u00E9 \u4E2D\u6587 \uD83D\uDE00', '&mnemonic', '%(title)s', 'x\u202Ey'];
  const lists = [
    [],
    ...statuses.map((s) => [{ title: 'Song A', status: s }]),
    statuses.map((s, i) => ({ title: `Track ${i}`, status: s })),
    [...statuses, ...statuses, ...statuses].map((s, i) => ({ title: `Many ${i}`, status: s })),
    ...titles.map((title) => [{ title, status: 'downloading' }]),
    [{ title: 'same', status: 'done' }, { title: 'same', status: 'done' }],
  ];
  const cases = lists.map((recent) => {
    initTray({ iconPath: 'C:/icon.png', getRecent: () => recent, actions });
    return {
      recent,
      menu: template.map((i) => (i.type === 'separator'
        ? { type: 'separator' }
        // Electron treats a missing `enabled` as enabled.
        : { label: i.label, enabled: i.enabled !== false, action: names.find((n) => actions[n] === i.click) ?? null })),
    };
  });
  return { tray: { tooltip: tooltips[0], tooltipsAreAllEqual: tooltips.every((t) => t === tooltips[0]), icon: trayArg.path, cases } };
}
