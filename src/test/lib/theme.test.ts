import { readFileSync } from 'node:fs';
import { applyPalette } from '@/lib/theme';
import { PALETTES, isPaletteName } from '@shared/types';

describe('applyPalette', () => {
  beforeEach(() => { delete document.documentElement.dataset.palette; localStorage.clear(); });

  it('sets data-palette and mirrors it to localStorage', () => {
    applyPalette('moss');
    expect(document.documentElement.dataset.palette).toBe('moss');
    expect(localStorage.getItem('ahg-palette')).toBe('moss');
  });

  it('falls back to aurora for an unknown id', () => {
    applyPalette('nope' as never);
    expect(document.documentElement.dataset.palette).toBe('aurora');
  });

  it('still applies when localStorage throws', () => {
    const spy = vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => { throw new Error('denied'); });
    expect(() => applyPalette('slate')).not.toThrow();
    expect(document.documentElement.dataset.palette).toBe('slate');
    spy.mockRestore();
  });
});

describe('palettes.css vs PALETTES', () => {
  const css = readFileSync('src/palettes.css', 'utf8');
  const blockOf = (id: string, mode: string) => {
    const i = css.indexOf(`[data-palette="${id}"][data-theme="${mode}"] {`);
    return i < 0 ? '' : css.slice(i, css.indexOf('}', i));
  };

  it('isPaletteName accepts known ids only', () => {
    expect(isPaletteName('ember')).toBe(true);
    expect(isPaletteName('__proto__')).toBe(false);
  });

  it.each(PALETTES.filter((p) => p.id !== 'aurora'))('$id defines both themes with a matching swatch accent', (p) => {
    for (const mode of ['dark', 'light'] as const) {
      const b = blockOf(p.id, mode);
      expect(b).toContain(`--color-accent: ${p.accent[mode]};`);
      for (const t of ['bg-primary', 'text-muted', 'accent-deep', 'on-accent', 'border-control']) {
        expect(b).toContain(`--color-${t}:`);
      }
    }
  });
});
