import { app } from 'electron';
import { readFileSync } from 'node:fs';
import path from 'node:path';

// Secrets are loaded by the main process at runtime, never cross the preload
// bridge, and are never logged. Missing file / missing keys degrade to empty
// strings rather than throwing, so a build with no secrets still runs.
//
// Two formats are read, .env first: it is the format people expect, it is one
// line per value, and it is trivially swapped without touching JSON syntax. The
// legacy JSON is still read so an existing install keeps working untouched.
//
// A note on what this does and does not buy: both files ship inside the packaged
// app, so this is organisation, not protection. Anyone with the binary can read
// either one. The value here is that the token is swappable without a rebuild —
// which is exactly what makes rotating it cheap when it leaks.
export interface Secrets {
  licenseGistId: string;
  githubToken: string;
  discordClientId: string;
  discordClientSecret: string;
  discordBotToken: string;
}

const EMPTY: Secrets = {
  licenseGistId: '',
  githubToken: '',
  discordClientId: '',
  discordClientSecret: '',
  discordBotToken: '',
};

/** .env key for each secret — SCREAMING_SNAKE, the convention people expect. */
const ENV_KEYS: Record<keyof Secrets, string> = {
  licenseGistId: 'AHG_LICENSE_GIST_ID',
  githubToken: 'AHG_GITHUB_TOKEN',
  discordClientId: 'AHG_DISCORD_CLIENT_ID',
  discordClientSecret: 'AHG_DISCORD_CLIENT_SECRET',
  discordBotToken: 'AHG_DISCORD_BOT_TOKEN',
};

const str = (v: unknown): string => (typeof v === 'string' ? v.trim() : '');

/**
 * Minimal KEY=VALUE parser — no dependency for a format this small.
 * Handles `#` comments, blank lines, `export ` prefixes, surrounding quotes, and
 * values containing `=`. Does NOT do interpolation: a token is a literal, and
 * expanding `$` inside one would corrupt it.
 */
export function parseEnv(text: string): Record<string, string> {
  const out: Record<string, string> = {};
  for (const line of text.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith('#')) continue;
    const eq = trimmed.indexOf('=');
    if (eq <= 0) continue;
    const key = trimmed.slice(0, eq).replace(/^export\s+/, '').trim();
    let value = trimmed.slice(eq + 1).trim();
    const quoted = value.length > 1 && ((value.startsWith('"') && value.endsWith('"')) || (value.startsWith("'") && value.endsWith("'")));
    if (quoted) value = value.slice(1, -1);
    // An unquoted trailing comment is a comment; inside quotes it is data.
    else value = value.replace(/\s+#.*$/, '').trim();
    if (key) out[key] = value;
  }
  return out;
}

/** Every location a secrets file may live, most specific first. */
function candidates(filename: string): string[] {
  const cwdCandidate = path.join(process.cwd(), 'config', filename); // projectRoot (dev)
  const resourcesCandidate = path.join(process.resourcesPath ?? '', 'config', filename); // packaged
  return [
    path.join(app.getPath('userData'), 'config', filename),
    // Packaged builds trust the deterministic resourcesPath location over cwd,
    // which a portable exe launched from an arbitrary folder doesn't control.
    ...(app.isPackaged ? [resourcesCandidate, cwdCandidate] : [cwdCandidate, resourcesCandidate]),
    path.join(path.dirname(process.execPath), 'resources', 'config', filename), // exeDir
    // A .env beside the exe / project root is where people put one by instinct.
    path.join(process.cwd(), filename),
    path.join(path.dirname(process.execPath), filename),
  ];
}

/** Load secrets: .env wins, then legacy secrets.json; first file that yields any value. */
export function loadSecrets(): Secrets {
  for (const file of candidates('.env')) {
    try {
      const env = parseEnv(readFileSync(file, 'utf-8'));
      const found = {
        licenseGistId: str(env[ENV_KEYS.licenseGistId]),
        githubToken: str(env[ENV_KEYS.githubToken]),
        discordClientId: str(env[ENV_KEYS.discordClientId]),
        discordClientSecret: str(env[ENV_KEYS.discordClientSecret]),
        discordBotToken: str(env[ENV_KEYS.discordBotToken]),
      };
      // A .env that parses but carries none of our keys is somebody else's file —
      // fall through rather than locking the app to an empty set.
      if (Object.values(found).some(Boolean)) return found;
    } catch {
      // missing / unreadable → try the next path
    }
  }

  for (const file of candidates('secrets.json')) {
    try {
      const raw = JSON.parse(readFileSync(file, 'utf-8')) as Record<string, unknown>;
      return {
        licenseGistId: str(raw.licenseGistId),
        githubToken: str(raw.githubToken),
        discordClientId: str(raw.discordClientId),
        discordClientSecret: str(raw.discordClientSecret),
        discordBotToken: str(raw.discordBotToken),
      };
    } catch {
      // missing / unreadable / invalid JSON → try the next path
    }
  }

  return { ...EMPTY };
}
