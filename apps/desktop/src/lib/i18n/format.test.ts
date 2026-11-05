import { describe, expect, it } from 'vitest';
import { formatBytes, formatCount, formatDuration, formatPercent } from './format';

describe('formatBytes', () => {
  it('renders bytes without decimals', () => {
    expect(formatBytes(0, { locale: 'en' })).toBe('0 B');
    expect(formatBytes(999, { locale: 'en' })).toBe('999 B');
  });

  it('uses binary units by default', () => {
    expect(formatBytes(1024, { locale: 'en' })).toBe('1.0 KiB');
    expect(formatBytes(1536, { locale: 'en' })).toBe('1.5 KiB');
    expect(formatBytes(1024 ** 3, { locale: 'en' })).toBe('1.0 GiB');
  });

  it('uses decimal units when asked', () => {
    expect(formatBytes(1000, { units: 'decimal', locale: 'en' })).toBe('1.0 kB');
    expect(formatBytes(1_500_000, { units: 'decimal', locale: 'en' })).toBe('1.5 MB');
  });

  it('follows locale separators', () => {
    expect(formatBytes(1500, { units: 'decimal', locale: 'de' })).toBe('1,5 kB');
    expect(formatBytes(1500, { units: 'decimal', locale: 'fr' })).toBe('1,5 kB');
  });

  it('drops decimals above 100', () => {
    expect(formatBytes(150 * 1024, { locale: 'en' })).toBe('150 KiB');
  });

  it('is defensive about bad input', () => {
    expect(formatBytes(Number.NaN)).toBe('—');
    expect(formatBytes(-1)).toBe('—');
  });

  it('marks approximate values', () => {
    expect(formatBytes(1024, { approximate: true, locale: 'en' })).toBe('~1.0 KiB');
  });
});

describe('formatPercent', () => {
  it('clamps and rounds', () => {
    expect(formatPercent(0.5, 'en')).toBe('50%');
    expect(formatPercent(1.5, 'en')).toBe('100%');
    expect(formatPercent(-0.2, 'en')).toBe('0%');
  });
});

describe('formatCount', () => {
  it('groups thousands', () => {
    expect(formatCount(1234567, 'en')).toBe('1,234,567');
    expect(formatCount(1234567, 'de')).toBe('1.234.567');
  });
});

describe('formatDuration', () => {
  it('picks a readable unit', () => {
    expect(formatDuration(500, 'en')).toBe('500 ms');
    expect(formatDuration(1500, 'en')).toBe('1.5 s');
    expect(formatDuration(63000, 'en')).toBe('1 min 3 s');
  });
});
