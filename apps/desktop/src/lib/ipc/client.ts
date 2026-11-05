/**
 * The only place that talks to the Rust side.
 *
 * Components and state modules call these functions; nothing else may import
 * `invoke` directly. This keeps payload shapes, error normalization and future
 * DTO renames in one file.
 */
import { invoke } from '@tauri-apps/api/core';
import type {
  ChildrenPageDto,
  NodeDto,
  ScanOptionsDto,
  ScanProgressDto,
  SnapshotMetaDto,
  SortDirectionDto,
  SortKeyDto,
  SystemInfoDto,
  TreemapDto,
  VolumeDto,
} from './types';

/** Normalized error from a command. */
export class IpcError extends Error {
  readonly code: string;
  readonly retryable: boolean;

  constructor(code: string, message: string, retryable = false) {
    super(message);
    this.name = 'IpcError';
    this.code = code;
    this.retryable = retryable;
  }
}

function normalizeError(raw: unknown): IpcError {
  if (typeof raw === 'string') {
    const [code, ...rest] = raw.split(':');
    return new IpcError(code?.trim() || 'E_UNKNOWN', rest.join(':').trim() || raw);
  }
  if (raw && typeof raw === 'object') {
    const value = raw as { code?: string; message?: string; retryable?: boolean };
    return new IpcError(value.code ?? 'E_UNKNOWN', value.message ?? 'Unknown error', value.retryable ?? false);
  }
  return new IpcError('E_UNKNOWN', String(raw));
}

async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(command, args);
  } catch (raw) {
    throw normalizeError(raw);
  }
}

export const ipc = {
  listVolumes: (): Promise<VolumeDto[]> => call('list_volumes'),

  startScan: (path: string, options: ScanOptionsDto): Promise<void> =>
    call('start_scan', { request: { path, options } }),

  cancelScan: (): Promise<void> => call('cancel_scan'),

  getScanProgress: (): Promise<ScanProgressDto> => call('get_scan_progress'),

  getSnapshotMeta: (): Promise<SnapshotMetaDto | null> => call('get_snapshot_meta'),

  getNode: (nodeId: number): Promise<NodeDto> => call('get_node', { nodeId }),

  getChildren: (args: {
    nodeId: number;
    sort: SortKeyDto;
    direction: SortDirectionDto;
    offset: number;
    limit: number;
  }): Promise<ChildrenPageDto> => call('get_children', args),

  getTreemap: (args: {
    nodeId: number;
    maxDepth: number;
    budget: number;
    width: number;
    height: number;
  }): Promise<TreemapDto> => call('get_treemap', args),

  getSystemInfo: (): Promise<SystemInfoDto> => call('get_system_info'),
};
