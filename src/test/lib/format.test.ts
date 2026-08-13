import { describe, it, expect } from 'vitest';
import { formatDuration, formatBytes } from '@/lib/format';

describe('formatDuration', () => {
  it('returns empty string for falsy input', () => {
    expect(formatDuration(0)).toBe('');
  });

  it('formats under an hour as m:ss', () => {
    expect(formatDuration(187)).toBe('3:07');
  });

  it('formats an hour or more as h:mm:ss', () => {
    expect(formatDuration(3723)).toBe('1:02:03');
  });
});

describe('formatBytes', () => {
  it('returns an em dash for zero or negative', () => {
    expect(formatBytes(0)).toBe('—');
    expect(formatBytes(-5)).toBe('—');
  });

  it('formats bytes without decimals', () => {
    expect(formatBytes(512)).toBe('512 B');
  });

  it('formats binary (1024-based) units', () => {
    expect(formatBytes(1536)).toBe('1.5 KB');
    expect(formatBytes(1024 * 1024 * 3.4)).toBe('3.4 MB');
  });

  it('rolls over into TB/PB instead of capping at GB (B17)', () => {
    expect(formatBytes(1024 ** 4 * 1.5)).toBe('1.5 TB');
    expect(formatBytes(1024 ** 5 * 2)).toBe('2.0 PB');
  });
});
