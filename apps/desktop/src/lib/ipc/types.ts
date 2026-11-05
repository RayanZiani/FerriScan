/**
 * IPC types mirrored from `crates/ferriscan-core/src/dto.rs`.
 *
 * Generated shape, checked by `cargo xtask contract`. Do not add hand-written
 * fields here: change the Rust DTO first, then regenerate.
 */

export type VolumeKind =
  | 'fixed'
  | 'removable'
  | 'network'
  | 'optical'
  | 'ram'
  | 'unknown';

export interface VolumeDto {
  id: string;
  mountPoint: string;
  label: string;
  filesystem: string;
  kind: VolumeKind;
  totalBytes: number;
  availableBytes: number;
  usedBytes: number;
  readOnly: boolean;
  writable: boolean;
  locked: boolean;
}

export type ScanState =
  | 'idle'
  | 'preparing'
  | 'scanning'
  | 'finalizing'
  | 'ready'
  | 'cancelling'
  | 'cancelled'
  | 'failed';

export interface ScanOptionsDto {
  includeHidden: boolean;
  sameFilesystemOnly: boolean;
  minFileSize: number;
  maxDepth: number | null;
  threads: number;
  excludes: string[];
  followSymlinks: boolean;
}

export interface ScanRequestDto {
  path: string;
  options: ScanOptionsDto;
}

export interface ScanProgressDto {
  state: ScanState;
  items: number;
  bytes: number;
  errors: number;
  currentPath: string | null;
  elapsedMs: number;
  itemsPerSecond: number;
}

export interface SnapshotMetaDto {
  id: string;
  rootPath: string;
  rootName: string;
  rootNodeId: number;
  status: 'complete' | 'partial' | 'cancelled';
  fileCount: number;
  dirCount: number;
  errorCount: number;
  logicalBytes: number;
  allocatedBytes: number;
  durationMs: number;
  maxDepth: number;
}

export interface NodeDto {
  id: number;
  name: string;
  parent: number | null;
  size: number;
  allocated: number;
  mtime: number;
  childCount: number;
  depth: number;
  isDir: boolean;
  isFile: boolean;
  isSymlink: boolean;
  hidden: boolean;
  hasError: boolean;
  extension: string;
  relativePath: string;
}

export type SortKeyDto = 'size' | 'name' | 'items' | 'modified' | 'type';
export type SortDirectionDto = 'ascending' | 'descending';

export interface ChildrenPageDto {
  parent: number;
  rows: NodeDto[];
  total: number;
  offset: number;
}

export interface SearchHitDto {
  id: number;
  name: string;
  path: string;
  size: number;
  isDir: boolean;
}

export interface TreemapDto {
  nodes: number[];
  xs: number[];
  ys: number[];
  ws: number[];
  hs: number[];
  depths: number[];
  sizes: number[];
}

export interface AppErrorDto {
  code: string;
  message: string;
  retryable: boolean;
}

export interface SystemInfoDto {
  platform: string;
  appVersion: string;
  locale: string;
  localeOverridden: boolean;
  theme: string;
  highContrast: boolean;
  reducedMotion: boolean;
  settingsPath: string;
}
