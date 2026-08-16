import { useState, useEffect, useCallback, type ReactNode } from 'react';
import { motion, useReducedMotion } from 'framer-motion';
import {
  Sun, Moon, SystemTheme, OpenFolder, Refresh, ChevronRight,
  ExternalLink, Deactivate, Power, Download, Transfer, Premium,
  type AppIcon,
} from '@/components/ui/icons';
import { useAppStore } from '@/store';
import { Button } from '@/components/ui/Button';
import { Toggle } from '@/components/ui/Toggle';
import { Select } from '@/components/ui/Select';
import { Input } from '@/components/ui/Input';
import { Card } from '@/components/ui/Card';
import { Chip } from '@/components/ui/Chip';
import { ProgressBar } from '@/components/ui/ProgressBar';
import { SegmentedCapsule, type SegmentOption } from '@/components/ui/SegmentedCapsule';
import type { ThemeName, YtdlpUpdateStatus } from '@shared/types';
import { AUDIO_FORMATS, AUDIO_QUALITIES, VIDEO_QUALITIES } from '@/constants';
import { useTierLocks, premiumCopy, basicCopy } from '@/lib/tier';

// ── constants ────────────────────────────────────────────────────────────────

const THEME_OPTIONS: readonly SegmentOption<ThemeName>[] = [
  { value: 'light', label: 'Light', icon: Sun },
  { value: 'dark',  label: 'Dark',  icon: Moon },
  { value: 'system', label: 'System', icon: SystemTheme },
];

const DAYS = ['Su', 'Mo', 'Tu', 'We', 'Th', 'Fr', 'Sa'] as const;
const DAY_NAMES = [
  'Sunday', 'Monday', 'Tuesday', 'Wednesday', 'Thursday', 'Friday', 'Saturday',
] as const;

type UpdatePhase = 'idle' | 'checking' | 'available' | 'downloading' | 'ready' | 'uptodate' | 'error';

/** The five states a status object can report, and the dot that says so. */
type Tone = 'idle' | 'busy' | 'ready' | 'ok' | 'error';
const DOT: Record<Tone, string> = {
  idle:  'bg-text-muted',
  busy:  'bg-warning',
  ready: 'bg-accent',
  ok:    'bg-success',
  error: 'bg-error',
};

// ── layout primitives (file-local) ───────────────────────────────────────────

/** One setting: what it is on the left, its control on the right.
 *  `label` is a node so a consequential row can carry an icon inline. */
function Row({ label, desc, children, className = '' }: {
  label: ReactNode; desc?: ReactNode; children: ReactNode; className?: string;
}) {
  return (
    <div className={`flex items-center justify-between gap-8 py-3.5 transition-colors duration-200 ${className}`}>
      <div className="min-w-0 flex-1">
        <p className="text-sm font-medium text-text-primary leading-snug">{label}</p>
        {/* text-secondary, not muted: muted fails 4.5:1 on the light theme's near-white surface. */}
        {desc && <p className="text-xs text-text-muted mt-0.5 leading-relaxed max-w-[54ch]">{desc}</p>}
      </div>
      <div className="shrink-0">{children}</div>
    </div>
  );
}

/** A group of rows. The heading is a plain sentence-case h2 with an optional
 *  clarifying line — it explains the group instead of just naming it, which is
 *  what lets some groups drop the heading entirely and vary the page's rhythm. */
function Section({ title, hint, children, className = 'mb-4' }: {
  title?: string; hint?: string; children: ReactNode; className?: string;
}) {
  return (
    <section className={className}>
      {title && (
        <div className="px-1 mb-2">
          <h2 className="text-[13px] font-semibold text-text-primary tracking-[-0.01em]">{title}</h2>
          {hint && <p className="text-xs text-text-secondary mt-0.5 max-w-[62ch] leading-relaxed">{hint}</p>}
        </div>
      )}
      <Card className="divide-y divide-border-soft px-5">{children}</Card>
    </section>
  );
}

/** A labelled control that sits in a row of controls rather than in a Row.
 *  The <label> wrapper is the accessible name, so no aria-label is needed. */
function Field({ label, children }: { label: string; children: ReactNode }) {
  return (
    <label className="flex flex-col gap-1.5">
      <span className="text-xs font-medium text-text-secondary">{label}</span>
      {children}
    </label>
  );
}

/** Not a setting — a thing that reports state and offers one action.
 *  Reads as an object (dot + name + version + live line) rather than a toggle row. */
function StatusObject({ name, tone, version, desc, footer, children }: {
  name: string; tone: Tone; version?: string; desc: string;
  footer?: ReactNode; children?: ReactNode;
}) {
  const reduced = useReducedMotion();
  const pulse = tone === 'busy' && !reduced;
  return (
    <div className="flex items-start justify-between gap-6 py-4">
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2.5">
          <motion.span
            aria-hidden
            className={`w-1.5 h-1.5 rounded-full shrink-0 transition-colors duration-200 ${DOT[tone]}`}
            animate={pulse ? { opacity: [1, 0.3, 1] } : { opacity: 1 }}
            transition={pulse ? { repeat: Infinity, duration: 1.4, ease: 'easeInOut' } : { duration: 0 }}
          />
          <p className="text-sm font-medium text-text-primary leading-snug">{name}</p>
          {version && <span className="font-mono text-[11px] text-text-secondary">{version}</span>}
        </div>
        <p className="text-xs text-text-muted mt-1 ml-4 leading-relaxed max-w-[54ch]">{desc}</p>
        {footer && <div className="mt-2.5 ml-4 max-w-[280px]">{footer}</div>}
      </div>
      {children && <div className="shrink-0">{children}</div>}
    </div>
  );
}

/** A destructive license action arms before it fires: the first click swaps the
 *  button for an explicit confirm pair, so the irreversible click is never the
 *  first one. (This is the pattern "move to another PC" already used.) */
function DangerAction({ icon, label, desc, armedDesc, confirmLabel, onConfirm }: {
  icon: AppIcon; label: string; desc: string; armedDesc: string;
  confirmLabel: string; onConfirm: () => Promise<unknown>;
}) {
  const [armed, setArmed] = useState(false);
  const reduced = useReducedMotion();

  return (
    <Row label={label} desc={armed ? armedDesc : desc}>
      {/* Keyed remount plays the enter on swap; no exit animation, so the row
          never collapses to zero width mid-transition. */}
      <motion.div
        key={armed ? 'confirm' : 'idle'}
        initial={reduced ? false : { opacity: 0, x: 6 }}
        animate={{ opacity: 1, x: 0 }}
        transition={{ duration: reduced ? 0 : 0.15, ease: [0.25, 0.46, 0.45, 0.94] }}
        className="flex items-center gap-2"
      >
        {armed ? (
          <>
            <Button variant="ghost" size="sm" onClick={() => setArmed(false)}>Cancel</Button>
            <Button
              variant="danger"
              size="sm"
              onClick={() => void onConfirm().then(() => setArmed(false))}
            >
              {confirmLabel}
            </Button>
          </>
        ) : (
          <Button variant="ghost" size="sm" icon={icon} onClick={() => setArmed(true)}>
            {label}
          </Button>
        )}
      </motion.div>
    </Row>
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
  const [cooldownUntil, setCooldownUntil] = useState<string | null>(null);

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

    // This tab is lazy, so the startup update check has usually already fired and
    // its event is gone. Read the state main kept instead of showing a stale
    // "check for updates" prompt over an update that was found minutes ago.
    void window.electronAPI.update.state().then((s) => {
      if (s.phase === 'idle') return;
      setUpdatePhase(s.phase);
      if (s.version) setUpdateVersion(s.version);
      if (typeof s.percent === 'number') setUpdatePercent(s.percent);
    });
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
    }
  }, []);

  const openSupport = useCallback(() => {
    void window.electronAPI.app.openSupport();
  }, []);

  const browseOutputDir = useCallback(async () => {
    const dir = await window.electronAPI.dialog.selectDir();
    if (dir) updateConfig({ outputDir: dir });
  }, [updateConfig]);

  // Every control below reads `config`, so there is nothing to draw without it —
  // but returning null drew an EMPTY PANE: the tab looked broken with no way to
  // tell whether it was still loading or had given up. App retries the load once;
  // if that also failed, say so instead of showing a void.
  if (!config) {
    return (
      <div className="mx-auto w-full max-w-[820px] px-6 py-16">
        <p role="status" className="text-sm text-text-secondary">
          Loading your settings…
        </p>
        <p className="mt-2 text-xs text-text-muted max-w-[54ch] leading-relaxed">
          If this doesn’t clear, your settings file couldn’t be read. Restart the app — your
          downloads and license are unaffected.
        </p>
      </div>
    );
  }

  const toggleDay = (d: number) => {
    updateConfig({
      scheduleDays: config.scheduleDays.includes(d)
        ? config.scheduleDays.filter((x) => x !== d)
        : [...config.scheduleDays, d],
    });
  };

  // "Where did my file go?" — the answer is the leaf folder, so the path is shown
  // whole (wrapping, never truncated) with the destination folder carrying the weight.
  const dirCut  = Math.max(config.outputDir.lastIndexOf('\\'), config.outputDir.lastIndexOf('/'));
  const dirHead = dirCut > 0 ? config.outputDir.slice(0, dirCut + 1) : '';
  const dirLeaf = dirCut > 0 ? config.outputDir.slice(dirCut + 1) : config.outputDir;

  const scheduleOn = locks.isPremium && config.scheduleEnabled;

  const engineDesc =
    engine.state === 'checking'    ? 'Checking for a newer yt-dlp…'
    : engine.state === 'available' ? `yt-dlp ${engine.latest} is available`
    : engine.state === 'downloading' ? 'Downloading and verifying the engine…'
    : engine.state === 'updated'   ? `Engine updated to ${engine.current || engine.latest}`
    : engine.state === 'uptodate'  ? 'yt-dlp is up to date'
    : engine.state === 'error'     ? (engine.message || 'Engine check failed — try again')
    : 'yt-dlp powers every download';

  const engineTone: Tone =
    engine.state === 'available' ? 'ready'
    : engine.state === 'checking' || engine.state === 'downloading' ? 'busy'
    : engine.state === 'uptodate' || engine.state === 'updated' ? 'ok'
    : engine.state === 'error' ? 'error'
    : 'idle';

  const updateDesc =
    updatePhase === 'checking'    ? 'Checking for updates…'
    : updatePhase === 'available' ? `v${updateVersion} is available`
    : updatePhase === 'downloading' ? 'Downloading update…'
    : updatePhase === 'ready'     ? `v${updateVersion} ready — restart to install`
    : updatePhase === 'uptodate'  ? "You're on the latest version"
    : updatePhase === 'error'     ? 'Update check failed — try again'
    : 'Check for a new version of AHG Universal Converter';

  const updateTone: Tone =
    updatePhase === 'available' || updatePhase === 'ready' ? 'ready'
    : updatePhase === 'checking' || updatePhase === 'downloading' ? 'busy'
    : updatePhase === 'uptodate' ? 'ok'
    : updatePhase === 'error' ? 'error'
    : 'idle';

  return (
    // The pane (viewport minus the 68px rail, minus whatever outer container
    // App installs) is what changes width here, so the rules below are
    // @container rules rather than viewport breakpoints.
    <div className="@container h-full overflow-y-auto">
      {/* One measure below @7xl, two columns above it. 820px is the reading
          measure for a single column of Rows; a 1600px single column would be
          the "settings alone on a huge canvas" complaint, and 1600px of two
          columns is ~750px each — wide enough that a Row's label never drifts
          away from its control. @7xl (a 1280px pane ≈ a 1350px window) is the
          first point where the second column clears ~560px, below which the
          label/control pair starts to collide. */}
      <div className="mx-auto w-full max-w-[820px] @7xl:max-w-[1600px] px-6 @4xl:px-10 py-10 pb-16">

        <header className="mb-7">
          <h1 className="text-h1 text-text-primary">
            Settings
          </h1>
          <p className="text-sm text-text-secondary mt-1.5">
            Changes save themselves — there's no save button.
          </p>
        </header>

        {/* Columns are assigned by hand, never balanced automatically: a group
            must never be split across columns, and hand-assignment also means
            the stacked order below @7xl is exactly the DOM order it has today.
            Left = what a download produces; right = the app, its engine, and
            the license. */}
        <div className="@7xl:flex @7xl:items-start @7xl:gap-5">
        <div className="min-w-0 @7xl:flex-1">

        {/* ── Destination ──────────────────────────────────────────────────────
            No heading: the one setting that answers a question the user is
            already asking, so it leads the page and shows its own answer. */}
        <Card className="px-5 py-5 mb-4">
          <div className="flex items-start justify-between gap-6">
            <div className="min-w-0">
              <p className="text-xs font-medium text-text-secondary">Saving to</p>
              <p
                className="mt-1.5 font-mono text-[13px] leading-relaxed break-all"
                title={config.outputDir}
              >
                {dirHead && <span className="text-text-secondary">{dirHead}</span>}
                <span className="text-text-primary font-medium">{dirLeaf || '—'}</span>
              </p>
            </div>
            <Button
              variant="ghost"
              size="sm"
              icon={OpenFolder}
              onClick={() => void browseOutputDir()}
              className="shrink-0"
            >
              Change
            </Button>
          </div>
        </Card>

        {/* ── Download defaults ─────────────────────────────────────────────── */}
        <Section
          title="Download defaults"
          hint="What a new download starts with. Each platform tab can still override them."
        >
          {/* A three-track grid, not flex-wrap: wrapping left the third control
              orphaned on its own line the moment the card dropped under ~570px
              (which is exactly what the second column is). Three equal tracks
              never wrap — the narrowest column this card can sit in still gives
              each track ~165px — so the row keeps one shape at every size. */}
          <div className="grid grid-cols-3 gap-x-6 gap-y-5 py-5">
            <Field label="Audio format">
              <Select
                value={locks.effectiveFormat(config.defaultFormat)}
                lockedValues={locks.lockedFormats}
                onLocked={(f) => locks.nudge(premiumCopy.losslessFormat(f.toUpperCase()))}
                onChange={(e) => updateConfig({ defaultFormat: e.target.value })}
                options={AUDIO_FORMATS}
                className="w-full max-w-[13rem]"
              />
            </Field>

            <Field label="Audio quality">
              <Select
                value={locks.effectiveAudioQuality(config.defaultQuality)}
                onChange={(e) => updateConfig({ defaultQuality: e.target.value })}
                options={AUDIO_QUALITIES}
                lockedValues={locks.lockedAudioQualities}
                onLocked={() => locks.nudge(premiumCopy.losslessQuality())}
                className="w-full max-w-[13rem]"
              />
            </Field>

            <Field label="Video quality">
              <Select
                value={locks.effectiveVideoQuality(config.defaultVideoQuality)}
                onChange={(e) =>
                  updateConfig({
                    defaultVideoQuality: e.target.value as typeof config.defaultVideoQuality,
                  })
                }
                options={VIDEO_QUALITIES}
                lockedValues={locks.lockedVideoQualities}
                onLocked={(v) => locks.nudge(premiumCopy.videoQuality(v))}
                className="w-full max-w-[13rem]"
              />
            </Field>
          </div>

          {/* Denser than the block above on purpose — three related yes/no rules
              about the finished file, read as a set rather than as three settings. */}
          <div className="divide-y divide-border-soft">
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
          </div>
        </Section>

        {/* ── Appearance & behaviour (two former sections, one card) ────────── */}
        <Section title="Appearance & behaviour">
          <Row label="Theme" desc="Light, dark, or follow the system preference">
            <SegmentedCapsule
              options={THEME_OPTIONS}
              value={config.theme}
              onChange={(t) => updateConfig({ theme: t })}
            />
          </Row>

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

          <Row label="Discord Rich Presence" desc="Show what you're downloading in your Discord status">
            <Toggle
              label="Discord Rich Presence"
              checked={config.discordRichPresence}
              onChange={(v) => updateConfig({ discordRichPresence: v })}
            />
          </Row>
        </Section>

        </div>{/* ── column 1 ends ── */}
        <div className="min-w-0 @7xl:flex-1">

        {/* ── Schedule ──────────────────────────────────────────────────────────
            The heading lives inside the card and carries the master switch, so
            the group boundary does real work: everything under it is inert until
            the switch is on, and says so by dimming rather than by disappearing
            (a basic-tier user still sees what Premium would give them). */}
        <Section className="mb-4">
          <div className="flex items-center justify-between gap-8 py-4">
            <div className="min-w-0">
              <h2 className="text-[13px] font-semibold text-text-primary tracking-[-0.01em]">
                Scheduled downloads
              </h2>
              <p className="text-xs text-text-secondary mt-0.5 max-w-[54ch] leading-relaxed">
                {locks.isPremium
                  ? 'Start the download queue automatically at a set time'
                  : 'Premium — unattended runs at a set time on chosen days'}
              </p>
            </div>
            {/* Toggling stays possible so the tap can explain itself; the switch
                never flips on basic, and the main-process scheduler also refuses. */}
            <Toggle
              label="Enable scheduler"
              checked={scheduleOn}
              onChange={(v) =>
                locks.isPremium
                  ? updateConfig({ scheduleEnabled: v })
                  : locks.nudge(premiumCopy.scheduler())
              }
            />
          </div>

          <div
            className={`divide-y divide-border-soft transition-opacity duration-200 ${
              config.scheduleEnabled ? '' : 'opacity-55'
            }`}
          >
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

            {/* The one setting here that reaches outside the app. It gets a warning
                field the moment it is armed, so it stops reading like a checkbox. */}
            <Row
              className={config.scheduleShutdown ? '-mx-5 px-5 bg-warning/10' : ''}
              label={
                <span className="inline-flex items-center gap-2">
                  {config.scheduleShutdown && <Power size={14} className="text-warning shrink-0" />}
                  Shutdown after queue
                </span>
              }
              desc={
                config.scheduleShutdown
                  ? 'This PC will power off when the scheduled queue finishes'
                  : 'Power off the PC when scheduled downloads finish'
              }
            >
              <Toggle
                label="Shutdown after queue"
                checked={config.scheduleShutdown}
                onChange={(v) => updateConfig({ scheduleShutdown: v })}
                disabled={!config.scheduleEnabled}
              />
            </Row>
          </div>
        </Section>

        {/* ── Software (app updater + yt-dlp engine) ────────────────────────── */}
        <Section
          title="Software"
          hint="Two moving parts: the app itself, and the yt-dlp engine that does the downloading."
        >
          <StatusObject
            name="AHG Universal Converter"
            tone={updateTone}
            version={updateVersion ? `v${updateVersion}` : undefined}
            desc={updateDesc}
            footer={
              updatePhase === 'downloading' ? (
                <div className="flex items-center gap-3">
                  <ProgressBar
                    percent={updatePercent}
                    status="downloading"
                    label="Update download"
                    className="flex-1"
                  />
                  <span className="font-mono text-[11px] text-text-secondary w-9 text-right">
                    {updatePercent.toFixed(0)}%
                  </span>
                </div>
              ) : undefined
            }
          >
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
                icon={Refresh}
                loading={updatePhase === 'checking'}
                disabled={updatePhase === 'checking'}
                onClick={checkForUpdates}
              >
                Check for updates
              </Button>
            )}
          </StatusObject>

          <StatusObject
            name="Download engine"
            tone={engineTone}
            version={engine.current || undefined}
            desc={engineDesc}
          >
            <Button
              variant={engine.state === 'available' ? 'primary' : 'ghost'}
              size="sm"
              icon={engine.state === 'available' ? Download : Refresh}
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
          </StatusObject>

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

        {/* ── Advanced — present, not prominent ─────────────────────────────────
            Native <details>: the disclosure is keyboard- and AT-correct for free,
            and opening it changes no height that needs animating. */}
        <details className="group mb-4">
          <summary className="list-none [&::-webkit-details-marker]:hidden cursor-pointer flex items-center gap-2 px-1 py-2 rounded-md text-[13px] font-semibold text-text-secondary hover:text-text-primary">
            <ChevronRight
              size={14}
              className="shrink-0 transition-transform duration-200 ease-out-quart group-open:rotate-90"
            />
            Advanced
            <span className="font-normal text-xs text-text-secondary">
              Network and shortcut settings most people never touch
            </span>
          </summary>

          <Card className="divide-y divide-border-soft px-5 mt-2">
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
          </Card>
        </details>

        {/* ── Plan & license ───────────────────────────────────────────────── */}
        <Section title="Plan & license" className="mb-3">
          <StatusObject
            name="Your plan"
            tone={plan === 'premium' ? 'ok' : 'idle'}
            version={plan === 'premium' ? 'Premium' : 'Basic'}
            // Basic states what it INCLUDES, not what it withholds. The same
            // facts, read as a capability rather than a list of confiscations.
            desc={plan === 'premium' ? 'Every feature is unlocked on this machine' : basicCopy.summary}
          >
            {plan !== 'premium' && (
              <Button variant="primary" size="sm" icon={Premium} onClick={locks.openUpgrade}>
                Upgrade
              </Button>
            )}
          </StatusObject>
        </Section>

        {/* Its own framed surface: these two actions end this machine's access, and
            a border in the error hue is the honest way to say so. */}
        {lic.activated && (
          <Card tone="danger" className="px-5 divide-y divide-border-soft mb-4">
            <DangerAction
              icon={Deactivate}
              label="Deactivate license"
              desc="Release this machine so you can activate on another device"
              armedDesc="This machine loses Premium immediately. Reactivating needs the key again."
              confirmLabel="Confirm deactivate"
              onConfirm={deactivate}
            />
            <DangerAction
              icon={Transfer}
              label="Move to another PC"
              desc="Deactivate here and release the key so it can be activated on a different computer"
              armedDesc="This PC deactivates now. The key becomes claimable on the new machine. The next transfer is allowed after 7 days."
              confirmLabel="Confirm move"
              onConfirm={transfer}
            />
          </Card>
        )}

        {cooldownUntil && (
          <p
            role="status"
            className="text-xs rounded-lg border border-warning/30 bg-warning/10 text-text-primary px-3.5 py-2.5 leading-relaxed mb-4"
          >
            Transfer unavailable until {new Date(cooldownUntil).toLocaleDateString()} — a license
            can be moved to another PC once every 7 days.
          </p>
        )}

        </div>{/* ── column 2 ends ── */}
        </div>

        {/* Footer: the two things that are neither settings nor status. */}
        <div className="flex flex-wrap items-center justify-between gap-3 px-1 pt-2">
          {lic.machineId ? (
            <p className="font-mono text-[11px] text-text-secondary" title="What a key binds to on this computer">
              Machine ID <span className="text-text-primary">{lic.machineId}</span>
            </p>
          ) : (
            <span />
          )}
          <Button variant="ghost" size="sm" icon={ExternalLink} onClick={() => void openSupport()}>
            Get support
          </Button>
        </div>

      </div>
    </div>
  );
}
