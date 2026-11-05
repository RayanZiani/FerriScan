import { describe, expect, it } from 'vitest';
import { ensureContrast, resolveTheme } from './theme';

describe('resolveTheme', () => {
  it('follows the OS in system mode', () => {
    expect(resolveTheme('system', 'dark')).toBe('dark');
    expect(resolveTheme('system', 'light')).toBe('light');
  });

  it('honors an explicit override', () => {
    expect(resolveTheme('light', 'dark')).toBe('light');
    expect(resolveTheme('dark', 'light')).toBe('dark');
  });
});

describe('ensureContrast', () => {
  it('keeps a color that already passes', () => {
    expect(ensureContrast('#000000', '#ffffff')).toBe('#000000');
  });

  it('darkens a low-contrast color on a light surface', () => {
    const adjusted = ensureContrast('#ffff00', '#ffffff');
    expect(adjusted).not.toBe('#ffff00');
  });

  it('lightens a low-contrast color on a dark surface', () => {
    const adjusted = ensureContrast('#000033', '#0f1115');
    expect(adjusted).not.toBe('#000033');
  });

  it('returns the input for malformed colors', () => {
    expect(ensureContrast('nope', '#ffffff')).toBe('nope');
  });
});
