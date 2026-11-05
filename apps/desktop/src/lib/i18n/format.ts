/**
 * Locale-aware formatting for sizes, counts and dates.
 *
 * The engine returns raw integers; every human-readable number is produced here
 * so locale rules (separators, units, relative dates) stay in one place.
 */

export type Units = 'binary' | 'decimal';

const BINARY_UNITS = ['B', 'KiB', 'MiB', 'GiB', 'TiB', 'PiB'] as const;
const DECIMAL_UNITS = ['B', 'kB', 'MB', 'GB', 'TB', 'PB'] as const;

export interface FormatBytesOptions {
  units?: Units;
  locale?: string;
  /** Digits after the decimal point; defaults to 1 for units above bytes. */
  digits?: number;
  /** Prefixes the result with `~` for approximated values. */
  approximate?: boolean;
}

/** Formats a byte count with locale-aware separators and unit names. */
export function formatBytes(
  bytes: number,
  { units = 'binary', locale = 'en', digits, approximate = false }: FormatBytesOptions = {},
): string {
  if (!Number.isFinite(bytes) || bytes < 0) return '—';
  const table = units === 'binary' ? BINARY_UNITS : DECIMAL_UNITS;
  const base = units === 'binary' ? 1024 : 1000;

  let value = bytes;
  let index = 0;
  while (value >= base && index < table.length - 1) {
    value /= base;
    index += 1;
  }

  const places = digits ?? (index === 0 ? 0 : value >= 100 ? 0 : 1);
  const formatted = new Intl.NumberFormat(locale, {
    minimumFractionDigits: places,
    maximumFractionDigits: places,
  }).format(value);

  const prefix = approximate ? '~' : '';
  return `${prefix}${formatted} ${table[index]}`;
}

/** Formats an integer count with grouping. */
export function formatCount(value: number, locale = 'en'): string {
  if (!Number.isFinite(value)) return '—';
  return new Intl.NumberFormat(locale).format(value);
}

/** Formats a percentage with one decimal at most. */
export function formatPercent(ratio: number, locale = 'en'): string {
  if (!Number.isFinite(ratio)) return '—';
  return new Intl.NumberFormat(locale, {
    style: 'percent',
    minimumFractionDigits: 0,
    maximumFractionDigits: 1,
  }).format(Math.min(Math.max(ratio, 0), 1));
}

/** Formats a Unix timestamp (seconds) as a short local date. */
export function formatDate(unixSeconds: number, locale = 'en'): string {
  if (!unixSeconds) return '—';
  return new Intl.DateTimeFormat(locale, { dateStyle: 'medium' }).format(
    new Date(unixSeconds * 1000),
  );
}

/** Formats a Unix timestamp as a localized date and time. */
export function formatDateTime(unixSeconds: number, locale = 'en'): string {
  if (!unixSeconds) return '—';
  return new Intl.DateTimeFormat(locale, { dateStyle: 'medium', timeStyle: 'short' }).format(
    new Date(unixSeconds * 1000),
  );
}

/** Formats a duration in milliseconds as `1.4 s` or `2 min 3 s`. */
export function formatDuration(ms: number, locale = 'en'): string {
  if (!Number.isFinite(ms) || ms < 0) return '—';
  if (ms < 1000) return `${Math.round(ms)} ms`;
  const seconds = ms / 1000;
  if (seconds < 60) {
    return `${new Intl.NumberFormat(locale, { maximumFractionDigits: 1 }).format(seconds)} s`;
  }
  const minutes = Math.floor(seconds / 60);
  const rest = Math.round(seconds % 60);
  return `${minutes} min ${rest} s`;
}

/** Normalizes a relative path for display with the platform separator. */
export function formatPath(path: string, separator = '/'): string {
  return path.split(/[\\/]+/).filter(Boolean).join(separator);
}
