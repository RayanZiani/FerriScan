<script lang="ts">
  import { t } from '$lib/i18n';
  import { formatBytes, formatCount } from '$lib/i18n/format';
  import { session } from '$lib/state/session.svelte';
  import { settings } from '$lib/state/settings.svelte';

  const units = $derived(settings.value.binaryUnits ? 'binary' : 'decimal');
  const locale = $derived(settings.value.locale);
  const progress = $derived(session.progress);
  const meta = $derived(session.meta);
</script>

<footer class="status">
  {#if progress}
    <span class="phase">{t(`scan.${progress.state === 'ready' ? 'ready' : progress.state}`)}</span>
    <span>{t('scan.progress.items', { count: formatCount(progress.items, locale) })}</span>
    <span>{t('scan.progress.bytes', { size: formatBytes(progress.bytes, { units, locale }) })}</span>
    {#if progress.itemsPerSecond > 0}
      <span>{t('scan.progress.rate', { rate: formatCount(progress.itemsPerSecond, locale) })}</span>
    {/if}
    {#if progress.currentPath}
      <span class="path" title={progress.currentPath}>{progress.currentPath}</span>
    {/if}
    {#if session.isBusy}
      <button type="button" class="cancel" onclick={() => session.cancel()}>{t('scan.cancel')}</button>
    {/if}
  {:else if meta}
    <span>{formatBytes(meta.allocatedBytes, { units, locale })}</span>
    <span>{t('scan.summary.files')}: {formatCount(meta.fileCount, locale)}</span>
    <span>{t('scan.summary.folders')}: {formatCount(meta.dirCount, locale)}</span>
    {#if meta.errorCount > 0}
      <span class="errors">{t('scan.summary.errors')}: {formatCount(meta.errorCount, locale)}</span>
    {/if}
    <span class="path" title={meta.rootPath}>{meta.rootPath}</span>
  {:else}
    <span class="muted">{t('scan.empty')}</span>
  {/if}
</footer>

<style>
  .status {
    display: flex;
    align-items: center;
    gap: var(--fs-space-4);
    height: 26px;
    padding: 0 var(--fs-space-3);
    border-top: 1px solid var(--fs-border);
    background: var(--fs-surface-2);
    font-size: var(--fs-text-xs);
    color: var(--fs-text-muted);
    white-space: nowrap;
    overflow: hidden;
  }

  .phase {
    font-weight: 600;
    color: var(--fs-text);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .path {
    margin-left: auto;
    overflow: hidden;
    text-overflow: ellipsis;
    direction: rtl;
    text-align: right;
    color: var(--fs-text-faint);
    max-width: 40%;
  }

  .errors {
    color: var(--fs-warning);
  }

  .muted {
    color: var(--fs-text-faint);
  }

  .cancel {
    border: 1px solid var(--fs-border);
    background: var(--fs-surface);
    color: var(--fs-text);
    border-radius: var(--fs-radius-sm);
    padding: 0 8px;
    height: 18px;
    font-size: var(--fs-text-xs);
    cursor: pointer;
  }

  .cancel:hover {
    background: var(--fs-surface-3);
  }
</style>
