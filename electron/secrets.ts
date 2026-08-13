import { app } from 'electron';
import { readFileSync } from 'node:fs';
import path from 'node:path';

// LICENSE-ACTIVATION-SYSTEM.md §10: secrets live ONLY in config/secrets.json and
// are loaded by the main process at runtime. They never cross the preload bridge
// and are never logged. Missing file / missing keys degrade to empty strings.
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

const str = (v: unknown): string => (typeof v === 'string' ? v : '');

/** Resolve config/secrets.json from the §10 fallback chain; first that parses wins. */
export function loadSecrets(): Secrets {
  const cwdCandidate = path.join(process.cwd(), 'config', 'secrets.json'); // projectRoot (dev)
  const resourcesCandidate = path.join(process.resourcesPath ?? '', 'config', 'secrets.json'); // packaged
  const candidates = [
    path.join(app.getPath('userData'), 'config', 'secrets.json'),
    // Packaged builds trust the deterministic resourcesPath location over cwd,
    // which a portable exe launched from an arbitrary folder doesn't control.
    ...(app.isPackaged ? [resourcesCandidate, cwdCandidate] : [cwdCandidate, resourcesCandidate]),
    path.join(path.dirname(process.execPath), 'resources', 'config', 'secrets.json'), // exeDir
  ];
  for (const file of candidates) {
    try {
      const raw = JSON.parse(readFileSync(file, 'utf-8')) as Record<string, unknown>;
      // Missing keys fall through to '' via str() — never throw, never log values.
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
