import { ERROR_CODES } from '@shared/types';
import { ERROR_COPY, describeError } from '@/lib/errors';

describe('error copy', () => {
  it('has a cause and a non-empty remedy for every ErrorCode', () => {
    for (const code of ERROR_CODES) {
      expect(ERROR_COPY[code].cause.length).toBeGreaterThan(0);
      expect(ERROR_COPY[code].remedy.length).toBeGreaterThan(0);
    }
  });
  it('falls back to unknown', () => {
    expect(describeError(undefined)).toBe(describeError('unknown'));
  });
});
