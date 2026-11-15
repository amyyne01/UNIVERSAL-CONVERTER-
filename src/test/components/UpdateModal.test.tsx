import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { UpdateModal } from '@/components/UpdateModal';

// The modal subscribes through window.electronAPI; capture each callback so a test
// can fire the main-process events by hand.
type Cb = (d: never) => void;
const fire = (name: string, data: unknown) => {
  const mock = window.electronAPI[name as 'onUpdateAvailable'] as unknown as { mock: { calls: [Cb][] } };
  act(() => mock.mock.calls.at(-1)![0](data as never));
};

describe('UpdateModal', () => {
  beforeEach(() => localStorage.clear());

  // D4: dismissing the dialog while the update downloads used to suppress the
  // restart prompt for the rest of the session.
  it('still shows the restart prompt when the download finishes after the dialog was dismissed', async () => {
    render(<UpdateModal />);
    fire('onUpdateAvailable', { version: '9.9.9' });
    fire('onUpdateProgress', { percent: 40 });
    expect(screen.getByRole('dialog')).toBeInTheDocument();
    fireEvent.keyDown(screen.getByRole('dialog'), { key: 'Escape' });
    await waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
    fire('onUpdateDownloaded', { version: '9.9.9' });
    expect(await screen.findByRole('button', { name: /Restart/ })).toBeInTheDocument();
  });
});
