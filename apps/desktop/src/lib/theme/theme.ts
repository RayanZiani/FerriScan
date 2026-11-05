/**
 * Theme resolution.
 *
 * The OS is the default source of truth. The Rust side pushes theme changes as
 * events; the webview also watches `prefers-color-scheme` so a missing event
 * never leaves the UI stale.
 */

export type ThemeMode = 'system' | 'light' | 'dark';
export type ResolvedTheme = 'light' | 'dark';

const DARK_QUERY = '(prefers-color-scheme: dark)';

/** Returns the media query list, or `null` where `matchMedia` is unavailable. */
function darkQuery(): MediaQueryList | null {
  if (typeof window === 'undefined' || typeof window.matchMedia !== 'function') return null;
  return window.matchMedia(DARK_QUERY);
}

export function systemTheme(): ResolvedTheme {
  return darkQuery()?.matches ? 'dark' : 'light';
}

export function resolveTheme(mode: ThemeMode, osTheme: ResolvedTheme = systemTheme()): ResolvedTheme {
  return mode === 'system' ? osTheme : mode;
}

/** Applies the theme to the document root. */
export function applyTheme(theme: ResolvedTheme): void {
  document.documentElement.dataset.theme = theme;
}

/** Applies the persisted density preference. */
export function applyDensity(compact: boolean): void {
  document.documentElement.dataset.density = compact ? 'compact' : 'comfortable';
}

/** Applies high-contrast styling from the OS preference. */
export function applyContrast(highContrast: boolean): void {
  document.documentElement.dataset.contrast = highContrast ? 'high' : 'normal';
}

/**
 * Ensures `color` is readable on `background` for AA normal text.
 *
 * If contrast is insufficient, the color is blended toward black or white until
 * it passes 4.5:1. Used for the accent color inherited from Windows so a
 * saturated system accent never produces unreadable labels.
 */
export function ensureContrast(color: string, background: string, ratio = 4.5): string {
  const fg = parseHex(color);
  const bg = parseHex(background);
  if (!fg || !bg) return color;
  let current = fg;
  for (let step = 0; step < 20; step++) {
    if (contrastRatio(current, bg) >= ratio) return toHex(current);
    current = mix(current, luminance(bg) > 0.5 ? [0, 0, 0] : [255, 255, 255], 0.08);
  }
  return toHex(current);
}

function parseHex(hex: string): [number, number, number] | null {
  const value = hex.replace('#', '').trim();
  if (value.length !== 6) return null;
  const num = Number.parseInt(value, 16);
  if (Number.isNaN(num)) return null;
  return [(num >> 16) & 255, (num >> 8) & 255, num & 255];
}

function toHex(rgb: [number, number, number]): string {
  return `#${rgb.map((c) => Math.round(c).toString(16).padStart(2, '0')).join('')}`;
}

function channel(value: number): number {
  const c = value / 255;
  return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
}

function luminance(rgb: [number, number, number]): number {
  return 0.2126 * channel(rgb[0]) + 0.7152 * channel(rgb[1]) + 0.0722 * channel(rgb[2]);
}

function contrastRatio(a: [number, number, number], b: [number, number, number]): number {
  const la = luminance(a);
  const lb = luminance(b);
  const [hi, lo] = la > lb ? [la, lb] : [lb, la];
  return (hi + 0.05) / (lo + 0.05);
}

function mix(
  a: [number, number, number],
  b: [number, number, number],
  amount: number,
): [number, number, number] {
  return [
    a[0] + (b[0] - a[0]) * amount,
    a[1] + (b[1] - a[1]) * amount,
    a[2] + (b[2] - a[2]) * amount,
  ];
}

/** Subscribes to OS theme changes. Returns an unsubscribe function. */
export function watchSystemTheme(handler: (theme: ResolvedTheme) => void): () => void {
  const media = darkQuery();
  if (!media) return () => {};
  const listener = (event: MediaQueryListEvent) => handler(event.matches ? 'dark' : 'light');
  media.addEventListener('change', listener);
  return () => media.removeEventListener('change', listener);
}
