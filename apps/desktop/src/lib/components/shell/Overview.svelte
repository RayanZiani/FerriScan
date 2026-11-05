<script lang="ts">
  import { t } from '$lib/i18n';
  import { formatBytes, formatCount } from '$lib/i18n/format';
  import { navigation } from '$lib/state/navigation.svelte';
  import { session } from '$lib/state/session.svelte';
  import { settings } from '$lib/state/settings.svelte';

  const units = $derived(settings.value.binaryUnits ? 'binary' : 'decimal');
  const locale = $derived(settings.value.locale);
  const meta = $derived(session.meta);

  const cards = $derived(
    meta
      ? [
          { label: t('scan.summary.size'), value: formatBytes(meta.allocatedBytes, { units, locale }) },
          { label: t('scan.summary.files'), value: formatCount(meta.fileCount, locale) },
          { label: t('scan.summary.folders'), value: formatCount(meta.dirCount, locale) },
          { label: t('scan.summary.duration'), value: `${(meta.durationMs / 1000).toFixed(1)} s` },
        ]
      : [],
  );
</script>

<div class="overview">
  {#if !meta}
    <p class="placeholder">{t('scan.empty_hint')}</p>
  {:else}
    <div class="cards">
      {#each cards as card (card.label)}
        <div class="card">
          <span class="value">{card.value}</span>
          <span class="label">{card.label}</span>
        </div>
      {/each}
      {#if meta.errorCount > 0}
        <div class="card warning">
          <span class="value">{formatCount(meta.errorCount, locale)}</span>
          <span class="label">{t('scan.summary.errors')}</span>
        </div>
      {/if}
    </div>

    {#if navigation.crumbs.length > 0}
      <nav class="crumbs" aria-label={t('table.name')}>
        {#each navigation.crumbs as crumb, index (crumb.id)}
          {#if index > 0}<span class="sep" aria-hidden="true">›</span>{/if}
          <button type="button" onclick={() => navigation.open(crumb.id)}>
            {crumb.name || meta.rootName}
          </button>
        {/each}
      </nav>
    {/if}
  {/if}
</div>

<style>
  .overview {
    padding: var(--fs-space-3);
    border-bottom: 1px solid var(--fs-border);
    background: var(--fs-bg);
  }

  .cards {
    display: flex;
    gap: var(--fs-space-2);
    flex-wrap: wrap;
  }

  .card {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 108px;
    padding: var(--fs-space-2) var(--fs-space-3);
    border: 1px solid var(--fs-border);
    border-radius: var(--fs-radius);
    background: var(--fs-surface);
  }

  .card.warning {
    border-color: color-mix(in srgb, var(--fs-warning) 45%, var(--fs-border));
  }

  .value {
    font-size: var(--fs-text-lg);
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  .label {
    font-size: var(--fs-text-xs);
    color: var(--fs-text-muted);
    text-transform: uppercase;
    letter-spacing: 0.03em;
  }

  .crumbs {
    display: flex;
    align-items: center;
    gap: 2px;
    margin-top: var(--fs-space-3);
    font-size: var(--fs-text-sm);
    overflow-x: auto;
    white-space: nowrap;
  }

  .crumbs button {
    border: 0;
    background: none;
    color: var(--fs-text-muted);
    padding: 2px 4px;
    border-radius: var(--fs-radius-sm);
    cursor: pointer;
    max-width: 220px;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .crumbs button:hover {
    background: var(--fs-surface-2);
    color: var(--fs-text);
  }

  .crumbs button:last-of-type {
    color: var(--fs-text);
    font-weight: 600;
  }

  .sep {
    color: var(--fs-text-faint);
  }

  .placeholder {
    color: var(--fs-text-muted);
    font-size: var(--fs-text-sm);
    margin: 0;
  }
</style>
