import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { ShortcutsSheet } from '@/components/ShortcutsSheet';
import { useAppStore } from '@/store';

describe('ShortcutsSheet', () => {
  it('renders nothing while closed', () => {
    useAppStore.setState({ shortcutsOpen: false });
    render(<ShortcutsSheet />);
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('lists the shortcuts and closes on Escape', async () => {
    useAppStore.setState({ shortcutsOpen: true });
    render(<ShortcutsSheet />);
    expect(screen.getByRole('dialog', { name: 'Keyboard shortcuts' })).toBeInTheDocument();
    expect(screen.getByText('Open the command palette')).toBeInTheDocument();
    await userEvent.keyboard('{Escape}');
    expect(useAppStore.getState().shortcutsOpen).toBe(false);
  });

  it('keeps Tab inside the sheet and lands on Close', async () => {
    useAppStore.setState({ shortcutsOpen: true });
    render(<ShortcutsSheet />);
    await userEvent.tab();
    expect(screen.getByRole('button', { name: 'Close' })).toHaveFocus();
    await userEvent.tab();
    expect(screen.getByRole('button', { name: 'Close' })).toHaveFocus();
  });
});
