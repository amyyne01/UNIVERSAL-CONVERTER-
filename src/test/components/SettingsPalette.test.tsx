import { fireEvent, render, screen } from '@testing-library/react';
import { SettingsTab } from '@/components/tabs/SettingsTab';
import { useAppStore } from '@/store';
import { mergeExtras, PALETTES, type AppConfig } from '@shared/types';

const config = {
  ...mergeExtras(), outputDir: 'C:\d', defaultFormat: 'mp3', defaultQuality: '320',
  defaultVideoQuality: 'best', theme: 'dark', palette: 'aurora', scheduleDays: [], scheduleEnabled: false,
} as unknown as AppConfig;

describe('Settings > Palette swatches', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    delete document.documentElement.dataset.palette;
    useAppStore.setState({ config, plan: 'basic', notice: null });
  });

  it('is a radiogroup with every palette, one tab stop on the active one', () => {
    render(<SettingsTab />);
    const group = screen.getByRole('radiogroup', { name: 'Colour palette' });
    const radios = Array.from(group.querySelectorAll('[role="radio"]'));
    expect(radios).toHaveLength(PALETTES.length);
    expect(screen.getByRole('radio', { name: 'Aurora' })).toHaveAttribute('aria-checked', 'true');
    expect(radios.filter((r) => r.getAttribute('tabindex') === '0')).toHaveLength(1);
  });

  it('click selects, previews live and persists', () => {
    render(<SettingsTab />);
    fireEvent.click(screen.getByRole('radio', { name: 'Moss' }));
    expect(useAppStore.getState().config?.palette).toBe('moss');
    expect(document.documentElement.dataset.palette).toBe('moss');
    expect(window.electronAPI.config.update).toHaveBeenCalledWith({ palette: 'moss' });
  });

  it('arrow keys move selection and wrap', () => {
    render(<SettingsTab />);
    fireEvent.keyDown(screen.getByRole('radio', { name: 'Aurora' }), { key: 'ArrowRight' });
    expect(useAppStore.getState().config?.palette).toBe(PALETTES[1].id);
    fireEvent.keyDown(screen.getByRole('radio', { name: 'Aurora' }), { key: 'ArrowLeft' });
    expect(useAppStore.getState().config?.palette).toBe(PALETTES[PALETTES.length - 1].id);
  });
});
