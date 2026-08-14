import { describe, it, expect, vi, beforeEach } from 'vitest';
import path from 'node:path';

vi.mock('electron', () => ({
  app: { getPath: vi.fn(() => 'C:\\userData') },
}));

vi.mock('node:fs', () => {
  const readFileSync = vi.fn();
  return { default: { readFileSync }, readFileSync };
});

import { loadSecrets } from './secrets';
import { readFileSync } from 'node:fs';

const mockRead = vi.mocked(readFileSync);
const USERDATA_PATH = path.join('C:\\userData', 'config', 'secrets.json');
const PROJECT_PATH = path.join(process.cwd(), 'config', 'secrets.json');

const FULL = {
  licenseGistId: 'gist123',
  githubToken: 'tok_abc',
  discordClientId: 'dc_id',
  discordClientSecret: 'dc_secret',
  discordBotToken: 'dc_bot',
};

beforeEach(() => {
  vi.clearAllMocks();
});

describe('loadSecrets', () => {
  it('returns parsed secrets from the first existing path (userData wins)', () => {
    mockRead.mockImplementation((file) => {
      if (file === USERDATA_PATH) return JSON.stringify(FULL);
      throw new Error('ENOENT');
    });
    expect(loadSecrets()).toEqual(FULL);
  });

  it('falls through to a later path when earlier ones are missing', () => {
    mockRead.mockImplementation((file) => {
      if (file === USERDATA_PATH) throw new Error('ENOENT');
      if (file === PROJECT_PATH) return JSON.stringify(FULL);
      throw new Error('ENOENT');
    });
    expect(loadSecrets()).toEqual(FULL);
  });

  it('fills missing keys with empty strings', () => {
    mockRead.mockImplementation((file) => {
      if (file === USERDATA_PATH) return JSON.stringify({ githubToken: 'only_token' });
      throw new Error('ENOENT');
    });
    expect(loadSecrets()).toEqual({
      licenseGistId: '',
      githubToken: 'only_token',
      discordClientId: '',
      discordClientSecret: '',
      discordBotToken: '',
    });
  });

  it('returns all empty strings when no file exists (no throw)', () => {
    mockRead.mockImplementation(() => {
      throw new Error('ENOENT');
    });
    expect(loadSecrets()).toEqual({
      licenseGistId: '',
      githubToken: '',
      discordClientId: '',
      discordClientSecret: '',
      discordBotToken: '',
    });
  });
});

// ── .env ──────────────────────────────────────────────────────────────────────
// The format people actually expect for secrets, and the one the app now writes.
// It must WIN over the legacy JSON, or rotating a token in .env would silently
// change nothing while the old JSON kept being used.

import { parseEnv } from './secrets';

const USERDATA_ENV = path.join(path.dirname(path.dirname(USERDATA_PATH)), 'config', '.env');
const PROJECT_ENV = path.join(process.cwd(), 'config', '.env');

const ENV_TEXT = [
  '# comment line',
  '',
  'AHG_LICENSE_GIST_ID=gist_from_env',
  'export AHG_GITHUB_TOKEN=tok_from_env',
  'AHG_DISCORD_CLIENT_ID="dc_id_quoted"',
  "AHG_DISCORD_CLIENT_SECRET='dc_secret_quoted'",
  'AHG_DISCORD_BOT_TOKEN=dc_bot   # trailing comment',
].join('\n');

describe('loadSecrets from .env', () => {
  it('reads a .env and prefers it over secrets.json', () => {
    mockRead.mockImplementation((file) => {
      if (file === PROJECT_ENV) return ENV_TEXT;
      if (file === PROJECT_PATH) return JSON.stringify(FULL); // legacy present too
      throw new Error('ENOENT');
    });
    expect(loadSecrets()).toEqual({
      licenseGistId: 'gist_from_env',
      githubToken: 'tok_from_env',
      discordClientId: 'dc_id_quoted',
      discordClientSecret: 'dc_secret_quoted',
      discordBotToken: 'dc_bot',
    });
  });

  it('falls through to secrets.json when the .env carries none of our keys', () => {
    mockRead.mockImplementation((file) => {
      if (file === USERDATA_ENV) return 'SOMETHING_ELSE=1\n';
      if (file === USERDATA_PATH) return JSON.stringify(FULL);
      throw new Error('ENOENT');
    });
    expect(loadSecrets()).toEqual(FULL);
  });

  it('still returns empty strings when neither format exists', () => {
    mockRead.mockImplementation(() => { throw new Error('ENOENT'); });
    expect(loadSecrets()).toEqual({
      licenseGistId: '', githubToken: '',
      discordClientId: '', discordClientSecret: '', discordBotToken: '',
    });
  });
});

describe('parseEnv', () => {
  it('handles comments, blanks, export, quotes and values containing =', () => {
    expect(parseEnv('# c\n\nA=1\nexport B=2\nC="x=y"\nD=\n')).toEqual({ A: '1', B: '2', C: 'x=y', D: '' });
  });

  it('does not expand $ — a token is a literal', () => {
    expect(parseEnv('T=ghp_$NOT_A_VAR')).toEqual({ T: 'ghp_$NOT_A_VAR' });
  });

  it('keeps a # inside quotes but strips an unquoted trailing comment', () => {
    expect(parseEnv('A="v#1"\nB=v2 # note')).toEqual({ A: 'v#1', B: 'v2' });
  });
});
