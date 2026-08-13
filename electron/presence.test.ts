import { describe, it, expect, vi, beforeEach } from 'vitest';
import { EventEmitter } from 'node:events';

// node:net mock: createConnection returns a fresh fake socket (EventEmitter + write/destroy).
vi.mock('node:net', () => {
  const createConnection = vi.fn(() => {
    const sock = new EventEmitter() as any;
    sock.write = vi.fn();
    sock.destroy = vi.fn();
    return sock;
  });
  return { default: { createConnection }, createConnection };
});

import { DiscordPresence } from './presence';
import { createConnection } from 'node:net';

/** The fake socket handed back by the Nth createConnection call. */
function socketAt(n = 0): any {
  return (createConnection as any).mock.results[n].value;
}

/** Decode a written Discord IPC frame: { op, json }. */
function decode(buf: Buffer): { op: number; json: any } {
  const op = buf.readInt32LE(0);
  const len = buf.readInt32LE(4);
  return { op, json: JSON.parse(buf.subarray(8, 8 + len).toString()) };
}

describe('DiscordPresence', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useRealTimers();
  });

  it('stays silent when clientId is empty', () => {
    new DiscordPresence().start('');
    expect(createConnection).not.toHaveBeenCalled();
  });

  it('connects to discord-ipc-0 and writes the op-0 handshake', () => {
    const p = new DiscordPresence();
    p.start('app-123');
    expect(createConnection).toHaveBeenCalledWith(expect.stringContaining('discord-ipc-0'));

    const sock = socketAt(0);
    sock.emit('connect');
    expect(sock.write).toHaveBeenCalledTimes(1);
    const { op, json } = decode(sock.write.mock.calls[0][0]);
    expect(op).toBe(0);
    expect(json).toEqual({ v: 1, client_id: 'app-123' });
  });

  it('debounces and coalesces activity into a single op-1 SET_ACTIVITY', () => {
    vi.useFakeTimers();
    const p = new DiscordPresence();
    p.start('cid');
    const sock = socketAt(0);
    sock.emit('connect'); // handshake write
    sock.write.mockClear();

    p.setActivity({ downloading: 1, queued: 2 });
    p.setActivity({ downloading: 3, queued: 4 }); // supersedes the first
    expect(sock.write).not.toHaveBeenCalled(); // nothing sent yet — debounced

    vi.advanceTimersByTime(3000);
    expect(sock.write).toHaveBeenCalledTimes(1); // both calls coalesced into one send
    const { op, json } = decode(sock.write.mock.calls[0][0]);
    expect(op).toBe(1);
    expect(json.cmd).toBe('SET_ACTIVITY');
    expect(json.args.activity.state).toContain('4'); // latest value won
  });
});
