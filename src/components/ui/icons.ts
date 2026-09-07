// The app's whole iconography, in one place.
//
// Every component imports its icons from here rather than from the icon package,
// so the set is a design decision made once instead of eighteen times — and a
// change of family, weight or metaphor is a single edit. Names describe the ROLE
// ("Retry", "OpenFolder"), not the drawing, so swapping the picture for a better
// one never touches a call site.
//
// Family: Phosphor. One weight app-wide ('regular', set via IconContext in
// App.tsx) — with ONE deliberate exception, below.
//
// NOTE: @phosphor-icons/react 2.1.10 marks these exports `@deprecated Use XIcon`,
// but it does not actually export the `*Icon` names yet — that rename lands in
// v3. Renaming now breaks the build. Do it when the package version does.
import { createElement, type ComponentProps } from 'react';
import {
  YoutubeLogo, SpotifyLogo, SoundcloudLogo, FilmSlate, type Icon,
} from '@phosphor-icons/react';

export {
  // Navigation & chrome
  House as Home,
  Queue as QueueIcon,
  GearSix as Settings,
  MagnifyingGlass as Search,
  CaretDown as ChevronDown,
  CaretRight as ChevronRight,
  X as Close,
  Minus as WindowMinimize,
  Square as WindowMaximize,
  Copy as WindowRestore,

  // Media & format
  MusicNote as Track,
  Playlist as Collection,
  Waveform as Audio,
  VideoCamera as Video,
  MonitorPlay as Resolution,

  // Actions
  DownloadSimple as Download,
  LinkSimple as Link,
  Clipboard as Paste,
  FolderOpen as OpenFolder,
  ArrowClockwise as Refresh,
  ArrowCounterClockwise as Retry,
  ArrowSquareOut as ExternalLink,
  ArrowsLeftRight as Transfer,
  ArrowUpRight as GoTo,
  ArrowRight as Forward,
  Trash as Remove,
  DotsSixVertical as DragHandle,
  ArrowLineUp as MoveToFront,
  Pause,
  Play,
  Power,
  SignOut as Deactivate,

  // Selection
  CheckSquare as CheckboxChecked,
  Square as CheckboxEmpty,
  Minus as CheckboxMixed,

  // State & feedback
  Check,
  CheckCircle as Success,
  XCircle as Cancel,
  WarningCircle as Alert,
  SpinnerGap as Loading,
  Clock,
  Lock,
  Sparkle as Premium,
  Key as LicenseKey,
  ChatCircle as Support,
  CalendarCheck as Scheduled,
  Rows as Batch,

  // Theme
  Sun,
  Moon,
  Monitor as SystemTheme,
} from '@phosphor-icons/react';

// ── The exception ────────────────────────────────────────────────────────────
// Platform marks are LOGOS, not UI glyphs. Every one of them is a solid shape in
// the wild, and at 14–19px an outline version reads as a smudge rather than as
// "this is Spotify" — the one thing these four have to do instantly. So they are
// pre-bound to `fill` here instead of at each of the three call sites (the nav
// rail, the source cards, the tab header), which keeps the rule in one place.
// A call site can still override by passing its own `weight`.
const brand = (mark: Icon): Icon =>
  ((props: ComponentProps<Icon>) => createElement(mark, { weight: 'fill', ...props })) as Icon;

export const YouTube = brand(YoutubeLogo);
export const Spotify = brand(SpotifyLogo);
export const SoundCloud = brand(SoundcloudLogo);
export const ShortForm = brand(FilmSlate);

export { IconContext } from '@phosphor-icons/react';
/** The component type every icon shares — props are `size`, `weight`, `color`. */
export type { Icon as AppIcon, IconWeight } from '@phosphor-icons/react';
