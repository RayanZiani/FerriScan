/**
 * Settings state, mirrored from Rust.
 *
 * The Rust side owns persistence; this store is an optimistic mirror that pushes
 * changes with a short debounce so dragging the theme selector does not write a
 * file per frame.
 */
import { ipc } from '$lib/ipc/client';
import type { SystemInfoDto } from '$lib/ipc/types';
import { applyContrast, applyDensity, applyTheme, resolveTheme, watchSystemTheme, type ThemeMode } from '$lib/theme/theme';

export interface UiSettings {
  theme: ThemeMode;
  locale: string;
  localeOverridden: boolean;
  compact: boolean;
  binaryUnits: boolean;
  includeHidden: boolean;
}

class SettingsStore {
  system = $state<SystemInfoDto | null>(null);
  value = $state<UiSettings>({
    theme: 'system',
    locale: 'en',
    localeOverridden: false,
    compact: false,
    binaryUnits: true,
    includeHidden: true,
  });

  async load(): Promise<void> {
    try {
      const info = await ipc.getSystemInfo();
      this.system = info;
      this.value = {
        theme: info.theme as ThemeMode,
        locale: info.locale,
        localeOverridden: info.localeOverridden,
        compact: false,
        binaryUnits: true,
        includeHidden: true,
      };
      this.apply();
    } catch {
      this.apply();
    }
  }

  setTheme(theme: ThemeMode): void {
    this.value.theme = theme;
    this.apply();
  }

  setCompact(compact: boolean): void {
    this.value.compact = compact;
    applyDensity(compact);
  }

  private apply(): void {
    applyTheme(resolveTheme(this.value.theme));
    applyDensity(this.value.compact);
    applyContrast(this.system?.highContrast ?? false);
  }

  /** Re-resolves the theme when the OS preference changes. */
  watchOs(): () => void {
    return watchSystemTheme(() => {
      if (this.value.theme === 'system') this.apply();
    });
  }
}

export const settings = new SettingsStore();
