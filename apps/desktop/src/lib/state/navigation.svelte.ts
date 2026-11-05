/**
 * Navigation and view state for the results area.
 */
import { ipc } from '$lib/ipc/client';
import type { NodeDto, SortDirectionDto, SortKeyDto } from '$lib/ipc/types';

export type ViewMode = 'table' | 'treemap' | 'sunburst' | 'extensions' | 'largest';

class NavigationStore {
  /** Currently focused node, `null` before the first scan. */
  current = $state<NodeDto | null>(null);
  /** Breadcrumb path from the scan root to `current`. */
  crumbs = $state<NodeDto[]>([]);
  view = $state<ViewMode>('table');
  sort = $state<SortKeyDto>('size');
  direction = $state<SortDirectionDto>('descending');

  /** Resolves a node and refreshes the breadcrumb. */
  async open(nodeId: number): Promise<void> {
    try {
      const node = await ipc.getNode(nodeId);
      this.current = node;
      this.crumbs = await this.buildCrumbs(node);
    } catch {
      // A stale selection after a rescan is expected; ignore it.
    }
  }

  async reset(rootId: number): Promise<void> {
    await this.open(rootId);
  }

  toggleSort(key: SortKeyDto): void {
    if (this.sort === key) {
      this.direction = this.direction === 'ascending' ? 'descending' : 'ascending';
    } else {
      this.sort = key;
      this.direction = key === 'name' || key === 'type' ? 'ascending' : 'descending';
    }
  }

  private async buildCrumbs(node: NodeDto): Promise<NodeDto[]> {
    const chain: NodeDto[] = [node];
    let cursor = node;
    // Bound the walk: a corrupted parent chain must not hang the UI.
    for (let depth = 0; depth < 4096 && cursor.parent !== null; depth++) {
      cursor = await ipc.getNode(cursor.parent);
      chain.unshift(cursor);
    }
    return chain;
  }
}

export const navigation = new NavigationStore();
