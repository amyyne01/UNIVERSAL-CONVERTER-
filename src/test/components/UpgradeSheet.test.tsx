import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { UpgradeSheet } from '@/components/UpgradeSheet';
import { Notice } from '@/components/Notice';
import { useAppStore } from '@/store';

describe('UpgradeSheet', () => {
  beforeEach(() => {
    useAppStore.setState({ plan: 'basic', isActivated: false, upgradeOpen: true, notice: null });
    vi.clearAllMocks();
  });

  it('shows both plans with the one-time price, and no monthly framing', () => {
    render(<UpgradeSheet onClose={vi.fn()} />);
    expect(screen.getByRole('region', { name: 'Basic plan' })).toBeInTheDocument();
    expect(screen.getByRole('region', { name: 'Premium plan' })).toBeInTheDocument();
    expect(screen.getAllByText('$3.99').length).toBeGreaterThan(0);
    expect(screen.getByText('one-time purchase')).toBeInTheDocument();
    expect(screen.queryByText(/per month|\/mo|yearly/i)).not.toBeInTheDocument();
  });

  it('states the real basic ceilings, so the cards cannot drift from BASIC_LIMITS', () => {
    render(<UpgradeSheet onClose={vi.fn()} />);
    expect(screen.getByText('Up to 1080p')).toBeInTheDocument();
    expect(screen.getByText('5 links at a time')).toBeInTheDocument();
    expect(screen.getByText('First 20 tracks')).toBeInTheDocument();
  });

  it('a verified key flips the store to premium and closes the sheet', async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    vi.mocked(window.electronAPI.license.activate).mockResolvedValue({ success: true, plan: 'premium' });

    render(<UpgradeSheet onClose={onClose} />);
    await user.click(screen.getByRole('button', { name: /already have a key/i }));
    await user.type(screen.getByLabelText('License key'), 'KEY-123');
    await user.click(screen.getByRole('button', { name: /unlock premium/i }));

    await waitFor(() => expect(useAppStore.getState().plan).toBe('premium'));
    expect(useAppStore.getState().isActivated).toBe(true);
    await waitFor(() => expect(onClose).toHaveBeenCalled(), { timeout: 3000 });
  });

  it('a rejected key surfaces the reason and leaves the tier alone', async () => {
    const user = userEvent.setup();
    vi.mocked(window.electronAPI.license.activate).mockResolvedValue({
      success: false,
      error: 'invalid',
      message: 'Invalid License Key',
    });

    render(<UpgradeSheet onClose={vi.fn()} />);
    await user.click(screen.getByRole('button', { name: /already have a key/i }));
    await user.type(screen.getByLabelText('License key'), 'NOPE');
    await user.click(screen.getByRole('button', { name: /unlock premium/i }));

    expect(await screen.findByRole('alert')).toHaveTextContent('Invalid License Key');
    expect(useAppStore.getState().plan).toBe('basic');
  });

  it('Escape closes the sheet', async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    render(<UpgradeSheet onClose={onClose} />);
    await user.keyboard('{Escape}');
    expect(onClose).toHaveBeenCalled();
  });
});

describe('Notice', () => {
  beforeEach(() => {
    useAppStore.setState({ plan: 'basic', upgradeOpen: false, notice: null });
  });

  it('renders nothing until a premium control is touched', () => {
    render(<Notice />);
    expect(screen.queryByRole('status')).not.toBeInTheDocument();
  });

  it('names the limit that was hit and routes on to the sheet', async () => {
    const user = userEvent.setup();
    render(<Notice />);

    useAppStore.getState().showPremiumNudge('2160p video is a Premium feature.');
    expect(await screen.findByRole('status')).toHaveTextContent('2160p video is a Premium feature');

    // The price belongs to the sheet, where the value is laid out beside it —
    // not to a toast that interrupts someone mid-task.
    const cta = screen.getByRole('button', { name: /see plans/i });
    expect(cta).not.toHaveTextContent('$');
    expect(screen.getByRole('status')).not.toHaveTextContent('$');

    await user.click(cta);
    expect(useAppStore.getState().upgradeOpen).toBe(true);
    expect(useAppStore.getState().notice).toBeNull();
  });

  it('an error notice states the problem and offers no upsell', async () => {
    render(<Notice />);
    useAppStore.getState().showError('“Aura” isn’t where it was saved.');

    const toast = await screen.findByRole('status');
    expect(toast).toHaveTextContent('isn’t where it was saved');
    expect(screen.queryByRole('button', { name: /premium/i })).not.toBeInTheDocument();
  });
});
