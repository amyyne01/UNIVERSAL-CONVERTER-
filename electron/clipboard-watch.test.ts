import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest';
import { ClipboardWatcher } from './clipboard-watch';

const POLL = 1500;

function makeWatcher(text: string, enabled = true) {
  let clip = text;
  const onDetect = vi.fn();
  const watcher = new ClipboardWatcher({
    isEnabled: () => enabled,
    onDetect,
    readClipboard: () => clip,
  });
  return { watcher, onDetect, setClip: (v: string) => { clip = v; } };
}

beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  vi.useRealTimers();
  vi.clearAllMocks();
});

describe('ClipboardWatcher', () => {
  it('notifies with the detection when the clipboard changes to a recognisable link', () => {
    const { watcher, onDetect, setClip } = makeWatcher('');
    watcher.start();
    setClip('https://youtu.be/dQw4w9WgXcQ');
    vi.advanceTimersByTime(POLL);
    expect(onDetect).toHaveBeenCalledTimes(1);
    expect(onDetect.mock.calls[0][0]).toMatchObject({ platform: 'youtube' });
    watcher.stop();
  });

  it('does not fire for text that is not a recognisable link', () => {
    const { watcher, onDetect, setClip } = makeWatcher('');
    watcher.start();
    setClip('just some copied text');
    vi.advanceTimersByTime(POLL);
    expect(onDetect).not.toHaveBeenCalled();
    watcher.stop();
  });

  it('does not re-offer the same link twice in a row', () => {
    const { watcher, onDetect, setClip } = makeWatcher('');
    watcher.start();
    setClip('https://youtu.be/dQw4w9WgXcQ');
    vi.advanceTimersByTime(POLL);
    vi.advanceTimersByTime(POLL); // clipboard unchanged
    expect(onDetect).toHaveBeenCalledTimes(1);
    watcher.stop();
  });

  it('does not re-offer whatever was already on the clipboard before start()', () => {
    const { watcher, onDetect } = makeWatcher('https://youtu.be/dQw4w9WgXcQ');
    watcher.start();
    vi.advanceTimersByTime(POLL);
    expect(onDetect).not.toHaveBeenCalled();
    watcher.stop();
  });

  it('never notifies while disabled', () => {
    let enabled = false;
    let clip = '';
    const onDetect = vi.fn();
    const watcher = new ClipboardWatcher({
      isEnabled: () => enabled,
      onDetect,
      readClipboard: () => clip,
    });
    watcher.start();
    clip = 'https://youtu.be/dQw4w9WgXcQ';
    vi.advanceTimersByTime(POLL);
    vi.advanceTimersByTime(POLL);
    expect(onDetect).not.toHaveBeenCalled();
    enabled = true;
    clip = 'https://youtu.be/anotherID1';
    vi.advanceTimersByTime(POLL);
    expect(onDetect).toHaveBeenCalledTimes(1);
    watcher.stop();
  });

  it('offers the same link again once the clipboard has changed away and back', () => {
    const { watcher, onDetect, setClip } = makeWatcher('');
    watcher.start();
    setClip('https://youtu.be/dQw4w9WgXcQ');
    vi.advanceTimersByTime(POLL);
    setClip('something else');
    vi.advanceTimersByTime(POLL);
    setClip('https://youtu.be/dQw4w9WgXcQ');
    vi.advanceTimersByTime(POLL);
    expect(onDetect).toHaveBeenCalledTimes(2);
    watcher.stop();
  });

  it('swallows a bad clipboard read instead of crashing the process', () => {
    const errSpy = vi.spyOn(console, 'error').mockImplementation(() => {});
    const onDetect = vi.fn();
    const watcher = new ClipboardWatcher({
      isEnabled: () => true,
      onDetect,
      readClipboard: () => { throw new Error('clipboard unavailable'); },
    });
    watcher.start();
    expect(() => vi.advanceTimersByTime(POLL)).not.toThrow();
    expect(onDetect).not.toHaveBeenCalled();
    expect(errSpy).toHaveBeenCalled();
    watcher.stop();
    errSpy.mockRestore();
  });

  it('stop() clears the timer so no further ticks happen', () => {
    const { watcher, onDetect, setClip } = makeWatcher('');
    watcher.start();
    watcher.stop();
    setClip('https://youtu.be/dQw4w9WgXcQ');
    vi.advanceTimersByTime(POLL * 2);
    expect(onDetect).not.toHaveBeenCalled();
  });
});
