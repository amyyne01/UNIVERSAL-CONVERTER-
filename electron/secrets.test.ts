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
