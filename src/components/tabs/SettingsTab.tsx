import { useState, useEffect, useCallback, type ReactNode } from 'react';
import {
  Sun, Moon, Monitor, FolderOpen, RefreshCw,
  ExternalLink, LogOut, Power, Download, ArrowRightLeft, Sparkles,
} from 'lucide-react';
import { useAppStore } from '@/store';
import { Button } from '@/components/ui/Button';
import { Toggle } from '@/components/ui/Toggle';
import { Select } from '@/components/ui/Select';
import { Input } from '@/components/ui/Input';
import { Card } from '@/components/ui/Card';
import { Chip } from '@/components/ui/Chip';
import { SegmentedCapsule, type SegmentOption } from '@/components/ui/SegmentedCapsule';
import type { ThemeName, YtdlpUpdateStatus } from '@shared/types';
import { AUDIO_FORMATS, AUDIO_QUALITIES, VIDEO_QUALITIES } from '@/constants';
import { useTierLocks } from '@/lib/tier';

// ── constants ────────────────────────────────────────────────────────────────

const THEME_OPTIONS: readonly SegmentOption<ThemeName>[] = [
  { value: 'light', label: 'Light', icon: Sun },
  { value: 'dark',  label: 'Dark',  icon: Moon },
  { value: 'system', label: 'System', icon: Monitor },
];

const DAYS = ['Su', 'Mo', 'Tu', 'We', 'Th', 'Fr', 'Sa'] as const;
const DAY_NAMES = [
  'Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday',
] as const;

type UpdatePhase = 'idle' | 'checking' | 'available' | 'downloading' | 'ready' | 'uptodate' | 'error';

// ── layout primitives (file-local) ───────────────────────────────────────────

function Row({ label, desc, children }: { label: string; desc?: string; children: ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-8 py-4">
      <div className="min-w-0 flex-1">
        <p className="text-sm font-medium text-text-primary leading-snug">{label}</p>
        {desc && (
          <p className="text-xs text-text-muted mt-0.5 leading-relaxed">{desc}</p>
        )}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className="mb-6">
      <h2 className="font-mono text-[10.5px] font-medium uppercase tracking-[0.14em] text-text-muted mb-3 px-1">
        {title}
      </h2>
      <Card className="divide-y divide-border-soft px-5">
        {children}
      </Card>
    </div>
  );
}

// ── main component ───────────────────────────────────────────────────────────

export function SettingsTab() {
  const config       = useAppStore((s) => s.config);
  const updateConfig = useAppStore((s) => s.updateConfig);
  const locks        = useTierLocks();
  const plan         = useAppStore((s) => s.plan);

  const [updatePhase,   setUpdatePhase]   = useState<UpdatePhase>('idle');
  const [updateVersion, setUpdateVersion] = useState('');
  const [updatePercent, setUpdatePercent] = useState(0);

  // #26 engine (yt-dlp) self-update
  const [engine, setEngine] = useState<YtdlpUpdateStatus>({ current: '', latest: '', state: 'idle' });

  const [lic, setLic] = useState<{ loaded: boolean; activated: boolean; machineId?: string }>({
    loaded: false,
    activated: false,
  });
  const [transferConfirm, setTransferConfirm] = useState(false);
  const [cooldownUntil, setCooldownUntil]     = useState<string | null>(null);

  // license check + update event listeners on mount
  useEffect(() => {
    void window.electronAPI.license.check().then((r) =>
      setLic({ loaded: true, activated: r.activated, machineId: r.machineId }),
    );

    const u1 = window.electronAPI.onUpdateAvailable(({ version }) => {
      setUpdateVersion(version);
      setUpdatePhase('available');
    });
    const u2 = window.electronAPI.onUpdateProgress(({ percent }) => {
      setUpdatePercent(percent);
      setUpdatePhase('downloading');
    });
    const u3 = window.electronAPI.onUpdateDownloaded(({ version }) => {
      setUpdateVersion(version);
      setUpdatePhase('ready');
    });
    const u4 = window.electronAPI.onUpdateNotAvailable(() => setUpdatePhase('uptodate'));
    const u5 = window.electronAPI.onUpdateError(() => setUpdatePhase('error'));
    const u6 = window.electronAPI.onYtdlpStatus(setEngine);
    // Cheap read: honours the updater's 24h TTL, so this only hits the network once a day.
    void window.electronAPI.ytdlp.version().then(setEngine);

    return () => { u1(); u2(); u3(); u4(); u5(); u6(); };
  }, []);

  const checkForUpdates = useCallback(() => {
    setUpdatePhase('checking');
    void window.electronAPI.update.check();
  }, []);

  const deactivate = useCallback(async () => {
    const res = await window.electronAPI.license.deactivate();
    if (res.success) setLic((l) => ({ ...l, activated: false }));
  }, []);

  const transfer = useCallback(async () => {
    const res = await window.electronAPI.license.release();
    if (res.success) {
      setLic((l) => ({ ...l, activated: false }));
    } else if (res.error === 'cooldown') {
      setCooldownUntil(res.eligibleAt ?? null);
      setTransferConfirm(false);
    }
  }, []);

  const openSupport = useCallback(() => {
    void window.electronAPI.app.openSupport();
  }, []);

  const browseOutputDir = useCallback(async () => {
    const dir = await window.electronAPI.dialog.selectDir();
    if (dir) updateConfig({ outputDir: dir });
  }, [updateConfig]);

  if (!config) return null;

  const toggleDay = (d: number) => {
    updateConfig({
      scheduleDays: config.scheduleDays.includes(d)
        ? config.scheduleDays.filter((x) => x !== d)
        : [...config.scheduleDays, d],
    });
  };

  const engineDesc =
    engine.state === 'checking'    ? 'Checking for a newer yt-dlp…'
    : engine.state === 'available' ? `yt-dlp ${engine.latest} is available`
    : engine.state === 'downloading' ? 'Downloading and verifying the engine…'
    : engine.state === 'updated'   ? `Engine updated to ${engine.current || engine.latest}`
    : engine.state === 'uptodate'  ? 'yt-dlp is up to date'
    : engine.state === 'error'     ? (engine.message || 'Engine check failed — try again')
    : engine.current               ? `yt-dlp ${engine.current}`
    : 'yt-dlp powers every download';

  const updateDesc =
    updatePhase === 'checking'    ? 'Checking for updates…'
    : updatePhase === 'available' ? `v${updateVersion} is available`
    : updatePhase === 'downloading' ? 'Downloading update…'
    : updatePhase === 'ready'     ? `v${updateVersion} ready — restart to install`
    : updatePhase === 'uptodate'  ? "You're on the latest version"
    : updatePhase === 'error'     ? 'Update check failed — try again'
    : 'Check for a new version of AHG Universal Converter';

  return (
    <div className="h-full overflow-y-auto">
      <div className="max-w-[760px] mx-auto px-10 py-10">

        <h1 className="font-display text-[32px] font-semibold text-text-primary tracking-[-0.025em] leading-[1.1] mb-8">
          Settings
        </h1>

        {/* ── Downloads ───────────────────────────────────────────────────── */}
        <Section title="Downloads">
          <Row label="Output directory" desc="Where converted files are saved on disk">
            <div className="flex items-center gap-2">
              <span className="font-mono text-xs text-text-secondary max-w-[200px] truncate">
                {config.outputDir || '—'}
              </span>
              <Button
                variant="ghost"
                size="sm"
                icon={FolderOpen}
                onClick={() => void browseOutputDir()}
              >
                Browse
              </Button>
            </div>
          </Row>

          <Row label="Default format" desc="Audio container applied to new downloads">
            <Select
              label="Default format"
              value={locks.effectiveFormat(config.defaultFormat)}
              lockedValues={locks.lockedFormats}
              onLocked={(f) => locks.nudge(`${f.toUpperCase()} is lossless — a Premium format.`)}
              onChange={(e) => updateConfig({ defaultFormat: e.target.value })}
              options={AUDIO_FORMATS}
            />
          </Row>

          <Row label="Default quality" desc="Audio bitrate target">
            <Select
              label="Default quality"
              value={locks.effectiveAudioQuality(config.defaultQuality)}
              onChange={(e) => updateConfig({ defaultQuality: e.target.value })}
              options={AUDIO_QUALITIES}
              lockedValues={locks.lockedAudioQualities}
              onLocked={() => locks.nudge('Lossless audio is a Premium feature.')}
            />
          </Row>

          <Row label="Default video quality" desc="Maximum resolution for video downloads">
            <Select
              label="Default video quality"
              value={locks.effectiveVideoQuality(config.defaultVideoQuality)}
              onChange={(e) =>
                updateConfig({
                  defaultVideoQuality: e.target.value as typeof config.defaultVideoQuality,
                })
              }
              options={VIDEO_QUALITIES}
              lockedValues={locks.lockedVideoQualities}
              onLocked={(v) => locks.nudge(`${v} needs Premium — Basic downloads up to 1080p.`)}
            />
          </Row>

          <Row label="Skip existing" desc="Don't re-download files already present on disk">
            <Toggle
              label="Skip existing"
              checked={config.skipExisting}
              onChange={(v) => updateConfig({ skipExisting: v })}
            />
          </Row>

          <Row label="Embed thumbnail" desc="Write cover art into the audio file">
            <Toggle
              label="Embed thumbnail"
              checked={config.embedThumbnail}
              onChange={(v) => updateConfig({ embedThumbnail: v })}
            />
          </Row>

          <Row label="Embed metadata" desc="Write title, artist, and album tags">
            <Toggle
              label="Embed metadata"
              checked={config.embedMetadata}
              onChange={(v) => updateConfig({ embedMetadata: v })}
            />
          </Row>

          <Row
            label="Rate limit"
            desc="Bandwidth cap per download (e.g. 2M, 500K) — empty = unlimited"
          >
            <Input
              aria-label="Rate limit"
              value={config.rateLimit}
              onChange={(e) => updateConfig({ rateLimit: e.target.value })}
              placeholder="e.g. 2M"
              wrapClassName="w-36"
              className="font-mono"
            />
          </Row>

          <Row label="Proxy" desc="HTTP/SOCKS proxy for all outbound requests">
            <Input
              aria-label="Proxy"
              value={config.proxy}
              onChange={(e) => updateConfig({ proxy: e.target.value })}
              placeholder="http://host:port"
              wrapClassName="w-64"
              className="font-mono text-xs"
            />
          </Row>
        </Section>

        {/* ── Appearance ──────────────────────────────────────────────────── */}
        <Section title="Appearance">
          <Row label="Theme" desc="Light, dark, or follow the system preference">
            <SegmentedCapsule
              options={THEME_OPTIONS}
              value={config.theme}
              onChange={(t) => updateConfig({ theme: t })}
            />
          </Row>

        </Section>

        {/* ── Behavior ────────────────────────────────────────────────────── */}
        <Section title="Behavior">
          <Row
            label="Auto-paste from clipboard"
            desc="Detect and fill a URL when you switch to this window"
          >
            <Toggle
              label="Auto-paste from clipboard"
              checked={config.autoPaste}
              onChange={(v) => updateConfig({ autoPaste: v })}
            />
          </Row>

          <Row label="Desktop notifications" desc="Notify when a download completes or fails">
            <Toggle
              label="Desktop notifications"
              checked={config.showNotifications}
              onChange={(v) => updateConfig({ showNotifications: v })}
            />
          </Row>

          <Row label="Global hotkey" desc="Shortcut to bring this window to focus from anywhere">
            <Input
              aria-label="Global hotkey"
              value={config.globalHotkey}
              onChange={(e) => updateConfig({ globalHotkey: e.target.value })}
              placeholder="e.g. Ctrl+Shift+D"
              wrapClassName="w-48"
              className="font-mono text-xs"
            />
          </Row>

          <Row label="Discord Rich Presence" desc="Show what you're downloading in your Discord status">
            <Toggle
              label="Discord Rich Presence"
              checked={config.discordRichPresence}
              onChange={(v) => updateConfig({ discordRichPresence: v })}
            />
          </Row>
        </Section>

        {/* ── Schedule ────────────────────────────────────────────────────── */}
        <Section title="Schedule">
          <Row
            label="Enable scheduler"
            desc={
              locks.isPremium
                ? 'Start the download queue automatically at a set time'
                : 'Premium — unattended runs at a set time on chosen days'
            }
          >
            {/* Toggling stays possible so the tap can explain itself; the switch
                never flips on basic, and the main-process scheduler also refuses. */}
            <Toggle
              label="Enable scheduler"
              checked={locks.isPremium && config.scheduleEnabled}
              onChange={(v) =>
                locks.isPremium
                  ? updateConfig({ scheduleEnabled: v })
                  : locks.nudge('Scheduled downloads are a Premium feature.')
              }
            />
          </Row>

          <Row label="Start time" desc="Local time to trigger the queue">
            <Input
              aria-label="Start time"
              type="time"
              value={config.scheduleTime}
              onChange={(e) => updateConfig({ scheduleTime: e.target.value })}
              wrapClassName="w-36"
              className="font-mono"
              disabled={!config.scheduleEnabled}
            />
          </Row>

          <Row label="Active days" desc="Days of the week the schedule is active">
            <div className="flex gap-1.5">
              {DAYS.map((day, i) => (
                <Chip
                  key={day}
                  selected={config.scheduleDays.includes(i)}
                  onClick={() => toggleDay(i)}
                  disabled={!config.scheduleEnabled}
                  aria-label={DAY_NAMES[i]}
                  className="w-10 justify-center px-0 py-1.5 text-[11px] font-mono"
                >
                  {day}
                </Chip>
              ))}
            </div>
          </Row>

          <Row label="Shutdown after queue" desc="Power off the PC when scheduled downloads finish">
            <Toggle
              label="Shutdown after queue"
              checked={config.scheduleShutdown}
              onChange={(v) => updateConfig({ scheduleShutdown: v })}
              disabled={!config.scheduleEnabled}
            />
          </Row>
        </Section>

        {/* ── Updates ─────────────────────────────────────────────────────── */}
        <Section title="Updates">
          <Row label="Software update" desc={updateDesc}>
            <div className="flex items-center gap-3">
              {updatePhase === 'downloading' && (
                <span className="font-mono text-xs text-accent tabular-nums w-10 text-right">
                  {updatePercent.toFixed(0)}%
                </span>
              )}
              {updatePhase === 'ready' ? (
                <Button
                  variant="primary"
                  size="sm"
                  icon={Power}
                  onClick={() => void window.electronAPI.update.install()}
                >
                  Restart & install
                </Button>
              ) : updatePhase === 'available' ? (
                <Button
                  variant="primary"
                  size="sm"
                  icon={Download}
                  onClick={() => void window.electronAPI.update.download()}
                >
                  Download
                </Button>
              ) : (
                <Button
                  variant="ghost"
                  size="sm"
                  icon={RefreshCw}
                  loading={updatePhase === 'checking'}
                  disabled={updatePhase === 'checking'}
                  onClick={checkForUpdates}
                >
                  Check for updates
                </Button>
              )}
            </div>
          </Row>
        </Section>

        {/* ── Engine (#26 yt-dlp self-update) ─────────────────────────────── */}
        <Section title="Engine">
          <Row label="Download engine" desc={engineDesc}>
            <div className="flex items-center gap-3">
              {engine.current && (
                <span className="font-mono text-[11px] px-2.5 py-1 rounded-full border border-border-soft text-text-muted tabular-nums">
                  {engine.current}
                </span>
              )}
              <Button
                variant={engine.state === 'available' ? 'primary' : 'ghost'}
                size="sm"
                icon={engine.state === 'available' ? Download : RefreshCw}
                loading={engine.state === 'checking' || engine.state === 'downloading'}
                disabled={engine.state === 'checking' || engine.state === 'downloading'}
                onClick={() =>
                  void (engine.state === 'available'
                    ? window.electronAPI.ytdlp.update()
                    : window.electronAPI.ytdlp.version(true)
                  ).then(setEngine)
                }
              >
                {engine.state === 'available' ? 'Update engine' : 'Check for engine update'}
              </Button>
            </div>
          </Row>

          <Row
            label="Auto-update engine"
            desc="Keep yt-dlp current automatically — and repair it when a site changes and downloads start failing"
          >
            <Toggle
              label="Auto-update engine"
              checked={config.autoUpdateEngine}
              onChange={(v) => updateConfig({ autoUpdateEngine: v })}
            />
          </Row>
        </Section>

        {/* ── Plan ────────────────────────────────────────────────────────── */}
        <Section title="Plan">
          <Row
            label="Your plan"
            desc={
              plan === 'premium'
                ? 'Every feature is unlocked on this machine'
                : 'Free tier — 1080p, 5-link batches, 20 tracks per collection, lossy audio'
            }
          >
            <div className="flex items-center gap-2.5">
              <span
                className={`inline-flex items-center gap-1.5 px-3 py-1 rounded-full border text-xs font-semibold font-mono ${
                  plan === 'premium'
                    ? 'bg-success/10 border-success/30 text-success'
                    : 'bg-bg-hover border-border text-text-secondary'
                }`}
              >
                <span
                  className={`w-1.5 h-1.5 rounded-full shrink-0 ${
                    plan === 'premium' ? 'bg-success' : 'bg-text-muted'
                  }`}
                />
                {plan === 'premium' ? 'Premium' : 'Basic'}
              </span>
              {plan !== 'premium' && (
                <Button variant="primary" size="sm" icon={Sparkles} onClick={locks.openUpgrade}>
                  Upgrade
                </Button>
              )}
            </div>
          </Row>

          {lic.machineId && (
            <Row label="Machine ID" desc="What a key binds to on this computer">
              <span className="font-mono text-xs text-text-muted">{lic.machineId}</span>
            </Row>
          )}

          <Row label="Support" desc="Open the support page or report an issue">
            <Button
              variant="ghost"
              size="sm"
              icon={ExternalLink}
              onClick={() => void openSupport()}
            >
              Get support
            </Button>
          </Row>

          {lic.activated && (
            <Row
              label="Deactivate license"
              desc="Release this machine so you can activate on another device"
            >
              <Button
                variant="danger"
                size="sm"
                icon={LogOut}
                onClick={() => void deactivate()}
              >
                Deactivate
              </Button>
            </Row>
          )}

          {lic.activated && (
            <Row
              label="Move to another PC"
              desc={
                transferConfirm
                  ? 'This PC deactivates now. The key becomes claimable on the new machine. The next transfer is allowed after 7 days.'
                  : 'Deactivate here and release the key so it can be activated on a different computer'
              }
            >
              {transferConfirm ? (
                <div className="flex items-center gap-2">
                  <Button variant="ghost" size="sm" onClick={() => setTransferConfirm(false)}>
                    Cancel
                  </Button>
                  <Button variant="danger" size="sm" onClick={() => void transfer()}>
                    Confirm move
                  </Button>
                </div>
              ) : (
                <Button
                  variant="ghost"
                  size="sm"
                  icon={ArrowRightLeft}
                  onClick={() => setTransferConfirm(true)}
                >
                  Move to another PC
                </Button>
              )}
            </Row>
          )}

          {cooldownUntil && (
            <div className="py-3">
              <p
                role="status"
                className="text-xs rounded-lg border border-warning/30 bg-warning/10 text-warning px-3 py-2 leading-relaxed"
              >
                Transfer unavailable until {new Date(cooldownUntil).toLocaleDateString()} — a license
                can be moved to another PC once every 7 days.
              </p>
            </div>
          )}
        </Section>

      </div>
    </div>
  );
}
