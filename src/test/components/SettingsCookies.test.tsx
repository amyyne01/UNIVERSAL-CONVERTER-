import { fireEvent, render, screen } from '@testing-library/react';
import { SettingsTab } from '@/components/tabs/SettingsTab';
import { useAppStore } from '@/store';
import { mergeExtras, type AppConfig } from '@shared/types';

// Settings reads extras plus a handful of app fields; the rest is irrelevant here.
const config = {
  ...mergeExtras(), outputDir: 'C:\d', defaultFormat: 'mp3', defaultQuality: '320',
  defaultVideoQuality: 'best', theme: 'dark', scheduleDays: [], scheduleEnabled: false,
} as unknown as AppConfig;

describe('Settings > Import cookies.txt', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useAppStore.setState({ config, plan: 'basic', notice: null });
  });

  it('is gated on the basic plan: nudges, never opens the file dialog', async () => {
    render(<SettingsTab />);
    const b = screen.getByRole('button', { name: 'Choose file' });
    fireEvent.click(b);
    expect(window.electronAPI.cookies.import).not.toHaveBeenCalled();
    expect(useAppStore.getState().notice?.tone).toBe('premium');
  });

  it('premium: calls the import and reports success', async () => {
    useAppStore.setState({ plan: 'premium' });
    vi.mocked(window.electronAPI.cookies.import).mockResolvedValueOnce({ ok: true });
    render(<SettingsTab />);
    fireEvent.click(screen.getByRole('button', { name: 'Choose file' }));
    expect(window.electronAPI.cookies.import).toHaveBeenCalledTimes(1);
    expect(await screen.findByText(/Cookies imported/)).toBeInTheDocument();
  });
});
