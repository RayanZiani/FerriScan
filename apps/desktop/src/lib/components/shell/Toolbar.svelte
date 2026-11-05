<script lang="ts">
  import { t } from '$lib/i18n';
  import { navigation, type ViewMode } from '$lib/state/navigation.svelte';

  const views: { id: ViewMode; label: string }[] = [
    { id: 'table', label: 'view.table' },
    { id: 'treemap', label: 'view.treemap' },
    { id: 'sunburst', label: 'view.sunburst' },
    { id: 'extensions', label: 'view.extensions' },
    { id: 'largest', label: 'view.largest' },
  ];
</script>

<nav class="toolbar" aria-label={t('view.table')}>
  <div class="tabs" role="tablist">
    {#each views as view (view.id)}
      <button
        type="button"
        role="tab"
        class:active={navigation.view === view.id}
        aria-selected={navigation.view === view.id}
        onclick={() => (navigation.view = view.id)}
      >
        {t(view.label)}
      </button>
    {/each}
  </div>

  <div class="spacer"></div>
</nav>

<style>
  .toolbar {
    display: flex;
    align-items: center;
    gap: var(--fs-space-3);
    height: 38px;
    padding: 0 var(--fs-space-3);
    border-bottom: 1px solid var(--fs-border);
    background: var(--fs-surface);
  }

  .tabs {
    display: flex;
    gap: 2px;
  }

  .tabs button {
    height: 26px;
    padding: 0 var(--fs-space-3);
    border: 0;
    border-radius: var(--fs-radius-sm);
    background: transparent;
    color: var(--fs-text-muted);
    font-size: var(--fs-text-sm);
    cursor: pointer;
  }

  .tabs button:hover {
    background: var(--fs-surface-2);
    color: var(--fs-text);
  }

  .tabs button.active {
    background: var(--fs-accent-soft);
    color: var(--fs-accent);
    font-weight: 600;
  }

  :global(:root[data-theme='dark']) .tabs button.active {
    color: var(--fs-accent-hover);
  }

  .spacer {
    flex: 1;
  }
</style>
