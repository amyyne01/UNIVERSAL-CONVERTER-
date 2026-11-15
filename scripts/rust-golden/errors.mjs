// Golden file for downloader/errors.rs: the four error functions of
// electron/downloader.ts run over a corpus of yt-dlp stderr shapes.
import { readFileSync, writeFileSync } from 'node:fs';

export default async function ({ OUT, literals, ELECTRON }) {
  const d = await import('../../dist-electron/electron/downloader.js');

  // The needle tables are private to downloader.ts, so they are copied here by
  // class; the quoted strings of the source itself are harvested below as well,
  // so a needle added there is still exercised even if this list is stale.
  const AUTH = ['sign in', 'login required', 'members-only', 'members only', 'age-restricted', 'age restricted',
    'private', 'confirm your age', 'logged-in', 'logged in', 'authentication is required'];
  const GEO = ['geo', 'region', 'not available in your country'];
  const PERM = ['is unavailable', 'video unavailable', 'no longer available', 'removed', 'deleted', 'does not exist',
    'this video is not available', 'copyright', 'drm', 'is not a valid url', 'unsupported url', 'http error 404',
    'http error 410', 'live event will begin', 'premieres in', 'this live stream recording is not available'];
  const GATED = ['403: forbidden', 'requested format is not available', 'page needs to be reloaded'];
  const RETRY = ['timed out', 'timeout', 'temporar', 'http error 5', 'too many requests', 'connection reset',
    'reset by peer', 'connection refused', 'network', 'read error', 'unable to download', ...GATED];
  const CODES = [
    ['ffmpeg_missing', ['ffmpeg is not installed', 'ffmpeg not found', 'ffprobe and ffmpeg not found', 'ffmpeg-location']],
    ['disk_full', ['no space left', 'disk full', 'enospc', 'not enough space']],
    ['drm', ['drm']],
    ['age_restricted', ['age-restricted', 'age restricted', 'confirm your age']],
    ['private', ['private']],
    ['login_required', ['sign in', 'login required', 'members-only', 'members only', 'logged-in', 'logged in', 'authentication is required']],
    ['geo_blocked', ['geo', 'region', 'not available in your country']],
    ['rate_limited', ['too many requests', 'http error 429', 'rate-limit', 'rate limit']],
    ['unsupported', ['unsupported url', 'is not a valid url']],
    ['removed', ['http error 404', 'http error 410', 'removed', 'deleted', 'does not exist', 'no longer available', 'video unavailable', 'is unavailable', 'this video is not available', 'copyright', 'not found']],
  ];
  const needles = [...new Set([...AUTH, ...GEO, ...PERM, ...RETRY, ...CODES.flatMap(([, n]) => n)])];

  const src = readFileSync(`${ELECTRON}/electron/downloader.ts`, 'utf8');
  const body = src.slice(src.indexOf('export function classifyError'), src.indexOf('export class Downloader'));
  const harvested = [...body.matchAll(/'((?:[^'\\\n]|\\.)*)'/g)].map((m) => m[1]).filter((s) => s.length > 1);

  const TIMEOUT = 'Timed out reading that link — the site did not answer in time.';
  const cap = (s) => s[0].toUpperCase() + s.slice(1);
  const forms = (n) => [
    n, n.toUpperCase(), cap(n), `ERROR: [youtube] abc123: ${n}`,
    `WARNING: slow\nERROR: ${n.toUpperCase()}. Please retry\n`, `éè ${n} 日本語`, `xx${n}yy`,
    `  \t${n}\r\n`,
  ];

  const inputs = [];
  // Every string the three suites feed in.
  for (const f of ['electron/downloader.test.ts', 'electron/queue.test.ts', 'src/test/lib/errors.test.ts']) inputs.push(...literals(f));
  inputs.push(...harvested);
  // Every needle alone, in several cases and settings.
  for (const n of [...needles, ...harvested]) inputs.push(...forms(n));
  // Precedence: needles of one class against needles of each other class, both orders.
  const classes = [AUTH, GEO, PERM, RETRY, GATED];
  const reps = classes.map((c) => [c[0], c[c.length - 1], c[Math.floor(c.length / 2)]]);
  for (let i = 0; i < classes.length; i++) {
    for (let j = 0; j < classes.length; j++) {
      if (i === j) continue;
      for (const a of reps[i]) for (const b of reps[j]) inputs.push(`${a} ${b}`, `ERROR: ${a}\nERROR: ${b}`);
    }
  }
  // Precedence between the typed codes: each group's first and last needle against every other group's.
  for (const [ca, a] of CODES) {
    for (const [cb, b] of CODES) {
      if (ca === cb) continue;
      inputs.push(`${a[0]} / ${b[0]}`, `${b[b.length - 1]} / ${a[a.length - 1]}`);
    }
  }
  // Edge text.
  const long = 'x'.repeat(5000);
  inputs.push(
    '', ' ', '   ', '\n', '\r\n\r\n', '\t', ' ', '﻿', ' ', '　', '\u0085', '᠎', '​',
    TIMEOUT, TIMEOUT + ' extra', `  ${TIMEOUT}\n`, TIMEOUT.toLowerCase(), `ERROR: ${TIMEOUT}`, `${TIMEOUT}\nHTTP Error 403: Forbidden`,
    'Timed out reading that link', long, long + ' http error 404', 'http error 404 ' + long, `ERROR: ${long}`,
    'ERROR: ' + 'y'.repeat(199) + 'Z', 'ERROR: ' + 'y'.repeat(200), 'ERROR: ' + 'y'.repeat(201), 'ERROR: ' + 'é'.repeat(250),
    'ERROR: ' + '日'.repeat(250), 'ERROR: ' + '😀'.repeat(150), 'ERROR: x' + '😀'.repeat(100),
    'ERROR:', 'ERROR:   ', 'ERROR: ', 'error: lowercase prefix', '  ERROR: 　spaced', 'ERROR:ERROR: twice', 'XERROR: x',
    'WARNING: a\nERROR: boom\n\n  \n', 'ERROR: first\nsecond line\n', 'line one\r\nline two\r\n', 'a b', 'a\nb\n ',
    'ERROR: İstanbul unreachable', 'ERROR: Koc network', 'ERROR: ΣΊΣ', 'ERROR: PRIVATE VIDEO', 'ERROR: Private',
    'NETWORK', 'neTWork down', 'HTTP ERROR 5', 'Http Error 503', 'http error 5', 'http error 4', 'http error 40', 'http error 4040',
    'ERROR: [generic] Unable to download webpage: <urlopen error [Errno 11001] getaddrinfo failed>',
    'ERROR: [youtube] X1aFkAkFASk: Video unavailable. This video is not available in your country',
    'ERROR: unable to download video data: HTTP Error 403: Forbidden',
    'ERROR: [youtube] abc: Sign in to confirm your age. This video may be inappropriate for some users.',
    'ERROR: [youtube] abc: Requested format is not available. Use --list-formats for a list of available formats',
    'ERROR: [youtube] abc: The page needs to be reloaded.', 'ERROR: [Spotify] ffmpeg-location /bad/path does not exist',
    'OSError: [Errno 28] No space left on device', 'ERROR: unable to write data: [Errno 28] ENOSPC',
    'ERROR: Unsupported URL: https://example.com/page', 'ERROR: [generic] ‘x’ is not a valid URL',
    'ERROR: This live event will begin in 2 hours.', 'ERROR: Premieres in 3 days', 'ERROR: Something odd happened',
    'ERROR: [tiktok] 1: Unable to extract video\nERROR: Something odd happened   ',
  );

  // A lone surrogate is legal JavaScript but cannot be read back from JSON by Rust.
  const lone = /[\uD800-\uDBFF](?![\uDC00-\uDFFF])|(?<![\uD800-\uDBFF])[\uDC00-\uDFFF]/;
  const seen = new Set();
  const cases = [];
  let dropped = 0;
  for (const input of inputs) {
    if (typeof input !== 'string' || seen.has(input)) continue;
    seen.add(input);
    const out = {
      classify: d.classifyError(input), fallback: d.needsClientFallback(input),
      code: d.errorCodeOf(input), explain: d.explainError(input),
    };
    if (lone.test(out.explain) || lone.test(input)) { dropped++; continue; }
    cases.push({ input, ...out });
  }
  writeFileSync(`${OUT}/errors.json`, JSON.stringify(cases, null, 1) + '\n');
  console.log(`errors.json: ${cases.length} cases (${dropped} dropped for a split surrogate pair)`);
}
