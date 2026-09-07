// Screenshot capture pass: drive the real window and shoot every surface.
// Feeds the README and the Discord release announcement, so it is re-run before
// every release — see the Releasing section of CLAUDE.md.
// Runs on the Basic preset so the free-tier lines and the limit states are the
// ones a new user actually sees. Reports which shots it got and which it skipped,
// so a network-dependent miss can never be mistaken for a captured surface.
import { _electron as electron } from 'playwright';
import { mkdirSync } from 'node:fs';

mkdirSync('screenshots', { recursive: true });

const W = 1240, H = 820;
const RAIL = { home: 78, queue: 126, youtube: 190, spotify: 238, settings: -30 };
const done = [], skipped = [];

const app = await electron.launch({
  args: ['.'],
  cwd: process.cwd(),
  env: { ...process.env, AHG_DEV_PLAN: 'basic', NODE_ENV: 'development' },
});
const win = await app.firstWindow();
const errors = [];
win.on('pageerror', (e) => errors.push(String(e)));
win.on('console', (m) => { if (m.type() === 'error') errors.push(m.text().slice(0, 140)); });

await app.evaluate(async ({ BrowserWindow }, [width, height]) => {
  const b = BrowserWindow.getAllWindows()[0];
  b.setSize(width, height);
  b.center();
}, [W, H]);

const shot = async (name, ms = 450) => {
  await win.waitForTimeout(ms);
  await win.screenshot({ path: `screenshots/${name}.png`, animations: 'disabled' });
  done.push(name);
};
const go = async (y) => {
  const h = await win.evaluate(() => window.innerHeight);
  await win.mouse.click(34, y < 0 ? h + y : y);
  await win.waitForTimeout(650);
};
const theme = (t) => win.evaluate((v) => document.documentElement.setAttribute('data-theme', v), t);
const esc = async () => { await win.keyboard.press('Escape'); await win.waitForTimeout(400); };

// ── 00 boot splash — only exists for a moment, so shoot before anything else ──
await win.waitForLoadState('domcontentloaded');
try {
  await win.waitForSelector('#splash', { timeout: 1500, state: 'attached' });
  await shot('00-loading', 260);
} catch { skipped.push('00-loading (splash already dismissed)'); }

await win.waitForTimeout(3000);
await theme('dark');

// ── 01 dashboard ─────────────────────────────────────────────────────────────
await go(RAIL.home);
await shot('01-dashboard');

// ── 02 free batch limit: a 6th link dims Fetch and names the ceiling ─────────
const field = win.getByLabel('Paste a link');
const SIX = [
  'https://youtu.be/aaaaaaaaaaa', 'https://youtu.be/bbbbbbbbbbb',
  'https://youtu.be/ccccccccccc', 'https://youtu.be/ddddddddddd',
  'https://youtu.be/eeeeeeeeeee', 'https://youtu.be/fffffffffff',
].join(' ');
await field.fill(SIX);
await shot('02-batch-limit', 700);
await field.fill('');

// ── 03 queue ────────────────────────────────────────────────────────────────
await go(RAIL.queue);
await shot('03-queue');

// ── 04/05 platform tabs ─────────────────────────────────────────────────────
await go(RAIL.youtube);
await shot('04-youtube');

// 05 real search results — network + yt-dlp, so it is allowed to fail.
try {
  const search = win.getByPlaceholder(/Paste a YouTube link or search/i);
  await search.fill('lofi hip hop');
  await win.keyboard.press('Enter');
  await win.waitForTimeout(9000);
  const hasResults = await win.evaluate(() =>
    document.body.innerText.toLowerCase().includes('result') ||
    document.querySelectorAll('[data-result], article').length > 0);
  if (hasResults) await shot('05-youtube-results', 400);
  else skipped.push('05-youtube-results (no results returned)');
} catch (e) { skipped.push(`05-youtube-results (${String(e).slice(0, 60)})`); }

await go(RAIL.spotify);
await shot('06-spotify');

// ── 07 settings ─────────────────────────────────────────────────────────────
await go(RAIL.settings);
await shot('07-settings');

// ── 08 plans — opened from the Upgrade button in Settings ───────────────────
try {
  await win.getByRole('button', { name: /^Upgrade$/ }).click();
  await shot('08-plans', 900);
  await esc();
} catch (e) { skipped.push(`08-plans (${String(e).slice(0, 60)})`); }

// ── 09 command palette ──────────────────────────────────────────────────────
await go(RAIL.home);
await win.keyboard.press('Control+k');
await shot('09-palette', 600);
await esc();

// ── 10 light theme, same surface, so the pair is comparable ─────────────────
await theme('light');
await shot('10-light-dashboard', 700);

console.log(JSON.stringify({ captured: done, skipped, errors }, null, 1));
await app.close();
