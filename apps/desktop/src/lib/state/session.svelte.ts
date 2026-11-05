/**
 * Session state: current snapshot header, scan phase and volume list.
 *
 * Runes-only. Components read this store; only this module talks to the IPC
 * layer for session-level data.
 */
import { ipc, IpcError } from '$lib/ipc/client';
import { onScanDone, onScanError, onScanProgress } from '$lib/ipc/events';
import type { ScanProgressDto, SnapshotMetaDto, VolumeDto } from '$lib/ipc/types';

class SessionStore {
  volumes = $state<VolumeDto[]>([]);
  volumesLoading = $state(true);
  volumesError = $state<string | null>(null);
  selectedVolume = $state<string | null>(null);

  meta = $state<SnapshotMetaDto | null>(null);
  progress = $state<ScanProgressDto | null>(null);
  scanError = $state<string | null>(null);

  /** Whether the scan is currently active (not idle/ready). */
  isBusy = $derived(
    this.progress?.state === 'scanning' ||
      this.progress?.state === 'preparing' ||
      this.progress?.state === 'finalizing' ||
      this.progress?.state === 'cancelling',
  );

  async loadVolumes(): Promise<void> {
    this.volumesLoading = true;
    this.volumesError = null;
    try {
      this.volumes = await ipc.listVolumes();
      if (!this.selectedVolume && this.volumes.length > 0) {
        this.selectedVolume = this.volumes[0]?.id ?? null;
      }
    } catch (error) {
      this.volumesError = error instanceof IpcError ? error.message : String(error);
    } finally {
      this.volumesLoading = false;
    }
  }

  async scan(path: string): Promise<void> {
    this.scanError = null;
    try {
      await ipc.startScan(path, {
        includeHidden: true,
        sameFilesystemOnly: false,
        minFileSize: 0,
        maxDepth: null,
        threads: 0,
        excludes: [],
        followSymlinks: false,
      });
    } catch (error) {
      this.scanError = error instanceof IpcError ? error.message : String(error);
    }
  }

  async cancel(): Promise<void> {
    await ipc.cancelScan();
  }

  /** Subscribes to backend events. Returns a cleanup function. */
  async subscribe(): Promise<() => void> {
    const unlisteners = await Promise.all([
      onScanProgress((value) => {
        this.progress = value;
      }),
      onScanDone((value) => {
        this.meta = value;
        this.progress = null;
      }),
      onScanError((message) => {
        this.scanError = message;
        this.progress = null;
      }),
    ]);
    return () => {
      for (const unlisten of unlisteners) unlisten();
    };
  }
}

export const session = new SessionStore();
