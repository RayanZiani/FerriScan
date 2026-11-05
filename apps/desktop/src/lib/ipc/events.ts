/**
 * Event subscription helpers.
 *
 * Every listener returns its own unsubscribe function so components can clean up
 * in `$effect` without tracking the Tauri id themselves.
 */
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { ScanProgressDto, SnapshotMetaDto } from './types';

const progressUnlisteners: UnlistenFn[] = [];

export async function onScanProgress(
  handler: (progress: ScanProgressDto) => void,
): Promise<UnlistenFn> {
  const unlisten = await listen<ScanProgressDto>('scan://progress', (event) => handler(event.payload));
  progressUnlisteners.push(unlisten);
  return unlisten;
}

export async function onScanDone(
  handler: (meta: SnapshotMetaDto) => void,
): Promise<UnlistenFn> {
  return listen<SnapshotMetaDto>('scan://done', (event) => handler(event.payload));
}

export async function onScanError(handler: (message: string) => void): Promise<UnlistenFn> {
  return listen<string>('scan://error', (event) => handler(event.payload));
}

export function disposeAll(): void {
  for (const unlisten of progressUnlisteners.splice(0)) unlisten();
}
