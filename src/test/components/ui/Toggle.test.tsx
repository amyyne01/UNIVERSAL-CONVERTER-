import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { Toggle } from '@/components/ui/Toggle';

describe('Toggle', () => {
  it('renders a switch role element', () => {
    render(<Toggle checked={false} onChange={() => {}} />);
    expect(screen.getByRole('switch')).toBeInTheDocument();
  });

  it('label prop sets aria-label on the switch', () => {
    render(<Toggle label="Auto-paste" checked={false} onChange={() => {}} />);
    expect(screen.getByRole('switch', { name: 'Auto-paste' })).toBeInTheDocument();
  });

  it('calls onChange with true when toggled from off', async () => {
    const onChange = vi.fn();
    render(<Toggle label="Dark mode" checked={false} onChange={onChange} />);
    await userEvent.click(screen.getByRole('switch'));
    expect(onChange).toHaveBeenCalledWith(true);
  });

  it('calls onChange with false when toggled from on', async () => {
    const onChange = vi.fn();
    render(<Toggle label="Notifications" checked={true} onChange={onChange} />);
    await userEvent.click(screen.getByRole('switch'));
    expect(onChange).toHaveBeenCalledWith(false);
  });

  it('reflects checked state via aria-checked', () => {
    const { rerender } = render(<Toggle label="Test" checked={false} onChange={() => {}} />);
    expect(screen.getByRole('switch')).toHaveAttribute('aria-checked', 'false');
    rerender(<Toggle label="Test" checked={true} onChange={() => {}} />);
    expect(screen.getByRole('switch')).toHaveAttribute('aria-checked', 'true');
  });

  it('is disabled when disabled prop is set', () => {
    render(<Toggle checked={false} onChange={() => {}} disabled />);
    expect(screen.getByRole('switch')).toBeDisabled();
  });

  it('does not call onChange when disabled', async () => {
    const onChange = vi.fn();
    render(<Toggle checked={false} onChange={onChange} disabled />);
    await userEvent.click(screen.getByRole('switch'));
    expect(onChange).not.toHaveBeenCalled();
  });
});
