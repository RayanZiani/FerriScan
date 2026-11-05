<script lang="ts">
  import { t } from '$lib/i18n';
  import { formatBytes, formatDate, formatCount } from '$lib/i18n/format';
  import { ipc, IpcError } from '$lib/ipc/client';
  import type { NodeDto, SortKeyDto } from '$lib/ipc/types';
  import { navigation } from '$lib/state/navigation.svelte';
  import { settings } from '$lib/state/settings.svelte';

  const PAGE_SIZE = 200;

  let rows = $state<NodeDto[]>([]);
  let total = $state(0);
  let loading = $state(false);
  let error = $state<string | null>(null);
  let selected = $state<number | null>(null);

  const units = $derived(settings.value.binaryUnits ? 'binary' : 'decimal');
  const locale = $derived(settings.value.locale);

  async function load(): Promise<void> {
    const node = navigation.current;
    if (!node) {
      rows = [];
      total = 0;
      return;
    }
    loading = true;
    error = null;
    try {
      const page = await ipc.getChildren({
        nodeId: node.id,
        sort: navigation.sort,
        direction: navigation.direction,
        offset: 0,
        limit: PAGE_SIZE,
      });
      rows = page.rows;
      total = page.total;
    } catch (err) {
      error = err instanceof IpcError ? err.message : String(err);
      rows = [];
    } finally {
      loading = false;
    }
  }

  // Reload whenever the node or ordering changes.
  $effect(() => {
    void navigation.current?.id;
    void navigation.sort;
    void navigation.direction;
    void load();
  });

  function sortBy(key: SortKeyDto): void {
    navigation.toggleSort(key);
  }

  function ariaSort(key: SortKeyDto): 'ascending' | 'descending' | 'none' {
    if (navigation.sort !== key) return 'none';
    return navigation.direction;
  }

  function openRow(row: NodeDto): void {
    if (row.isDir) {
      void navigation.open(row.id);
    } else {
      selected = row.id;
    }
  }

  function onKeydown(event: KeyboardEvent, row: NodeDto): void {
    if (event.key === 'Enter') {
      event.preventDefault();
      openRow(row);
    }
  }
</script>

<div class="table-wrap">
  {#if !navigation.current}
    <p class="placeholder">{t('scan.empty')}</p>
  {:else if error}
    <p class="placeholder error">{error}</p>
  {:else if loading && rows.length === 0}
    <p class="placeholder">{t('table.loading')}</p>
  {:else if rows.length === 0}
    <p class="placeholder">{t('table.empty')}</p>
  {:else}
    <table>
      <thead>
        <tr>
          <th scope="col" class="name" aria-sort={ariaSort('name')}>
            <button type="button" onclick={() => sortBy('name')}>
              {t('table.name')}
            </button>
          </th>
          <th scope="col" class="num" aria-sort={ariaSort('size')}>
            <button type="button" onclick={() => sortBy('size')}>
              {t('table.size')}
            </button>
          </th>
          <th scope="col" class="num optional" aria-sort={ariaSort('items')}>
            <button type="button" onclick={() => sortBy('items')}>
              {t('table.items')}
            </button>
          </th>
          <th scope="col" class="optional" aria-sort={ariaSort('modified')}>
            <button type="button" onclick={() => sortBy('modified')}>
              {t('table.modified')}
            </button>
          </th>
        </tr>
      </thead>
      <tbody>
        {#each rows as row (row.id)}
          <tr
            class:selected={selected === row.id}
            class:hidden-file={row.hidden}
            tabindex="0"
            onclick={() => (selected = row.id)}
            ondblclick={() => openRow(row)}
            onkeydown={(event) => onKeydown(event, row)}
          >
            <td class="name">
              <span class="glyph" aria-hidden="true">{row.isDir ? '▸' : '·'}</span>
              <span class="label" title={row.relativePath}>{row.name}</span>
              {#if row.hasError}
                <span class="badge error" title={t('error.E_PERM')}>!</span>
              {/if}
            </td>
            <td class="num">{formatBytes(row.allocated, { units, locale })}</td>
            <td class="num optional">{formatCount(row.childCount, locale)}</td>
            <td class="optional">{formatDate(row.mtime, locale)}</td>
          </tr>
        {/each}
      </tbody>
    </table>
    {#if total > rows.length}
      <p class="more">{t('table.more', { count: formatCount(total - rows.length, locale) })}</p>
    {/if}
  {/if}
</div>

<style>
  .table-wrap {
    flex: 1;
    overflow: auto;
    background: var(--fs-bg);
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: var(--fs-text-md);
    table-layout: fixed;
  }

  thead th {
    position: sticky;
    top: 0;
    z-index: 1;
    background: var(--fs-surface-2);
    border-bottom: 1px solid var(--fs-border);
    font-weight: 600;
    font-size: var(--fs-text-xs);
    text-transform: uppercase;
    letter-spacing: 0.03em;
    color: var(--fs-text-muted);
    text-align: left;
    padding: 0;
  }

  thead th button {
    width: 100%;
    padding: 6px var(--fs-space-3);
    border: 0;
    background: none;
    color: inherit;
    font: inherit;
    text-align: inherit;
    cursor: pointer;
  }

  thead th button:hover {
    color: var(--fs-text);
  }

  thead th[aria-sort='ascending'] button::after {
    content: ' ▲';
  }

  thead th[aria-sort='descending'] button::after {
    content: ' ▼';
  }
  th.num,
  td.num {
    text-align: right;
  }

  th.num button {
    text-align: right;
  }

  th.name,
  td.name {
    width: auto;
  }

  tbody tr {
    height: var(--fs-row-height);
    border-bottom: 1px solid color-mix(in srgb, var(--fs-border) 45%, transparent);
  }

  tbody tr:hover {
    background: var(--fs-surface-2);
  }

  tbody tr.selected {
    background: var(--fs-accent-soft);
  }

  tbody tr:focus-visible {
    outline: 2px solid var(--fs-focus);
    outline-offset: -2px;
  }

  td {
    padding: 0 var(--fs-space-3);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .glyph {
    display: inline-block;
    width: 14px;
    color: var(--fs-text-faint);
  }

  .label {
    vertical-align: middle;
  }

  .hidden-file .label {
    color: var(--fs-text-muted);
  }

  .badge.error {
    display: inline-block;
    margin-left: var(--fs-space-2);
    padding: 0 4px;
    border-radius: var(--fs-radius-sm);
    background: var(--fs-danger-soft);
    color: var(--fs-danger);
    font-size: var(--fs-text-xs);
    font-weight: 700;
  }

  .optional {
    width: 150px;
  }

  .placeholder {
    padding: var(--fs-space-5);
    color: var(--fs-text-muted);
    text-align: center;
  }

  .placeholder.error {
    color: var(--fs-danger);
  }

  .more {
    padding: var(--fs-space-2) var(--fs-space-3);
    color: var(--fs-text-faint);
    font-size: var(--fs-text-sm);
  }

  @container (max-width: 700px) {
    .optional {
      display: none;
    }
  }
</style>
