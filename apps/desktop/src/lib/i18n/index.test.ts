import { describe, expect, it } from 'vitest';
import { errorMessage, messages, setLocale, t } from './index';

describe('t', () => {
  it('returns English strings by default', () => {
    expect(t('app.name')).toBe('FerriScan');
  });

  it('interpolates named placeholders', () => {
    expect(t('scan.progress.items', { count: 42 })).toBe('42 items');
  });

  it('leaves unknown placeholders intact', () => {
    expect(t('scan.progress.items', {})).toBe('{count} items');
  });

  it('falls back to the key for unknown strings', () => {
    expect(t('does.not.exist')).toBe('does.not.exist');
  });

  it('merges an override locale over English', () => {
    setLocale('fr', { 'app.name': 'FerriScan FR' });
    expect(t('app.name')).toBe('FerriScan FR');
    expect(t('nav.volumes')).toBe('Volumes');
    setLocale('en');
    expect(t('app.name')).toBe('FerriScan');
  });
});

describe('errorMessage', () => {
  it('maps stable codes to text', () => {
    expect(errorMessage('E_PERM')).toBe('Permission denied');
  });

  it('never returns undefined', () => {
    expect(typeof errorMessage('E_WHATEVER')).toBe('string');
  });
});

describe('messages', () => {
  it('has no empty values', () => {
    for (const [key, value] of Object.entries(messages())) {
      expect(value.trim(), key).not.toBe('');
    }
  });
});
