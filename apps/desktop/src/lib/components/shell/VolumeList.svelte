<script lang="ts">
  import { formatBytes, formatPercent } from '$lib/i18n/format';
  import { t } from '$lib/i18n';
  import { session } from '$lib/state/session.svelte';
  import { settings } from '$lib/state/settings.svelte';
  import ProgressBar from '$lib/components/common/ProgressBar.svelte';

  const volumes = $derived(session.volumes);
  const units = $derived(settings.value.binaryUnits ? 'binary' : 'decimal');
  const locale = $derived(settings.value.locale);

  function kindLabel(kind: string): string {
    return t(`kind.${kind}`);
  }

  function tone(used: number): 'accent' | 'warning' | 'danger' {
    if (used > 0.92) return 'danger';
    if (used > 0.8) return 'warning';
    return 'accent';
  }

  async function scan(id: string): Promise<void> {
    session.selectedVolume = id;
    await session.scan(id);
  }
</script>

<aside class="volumes" aria-label={t('volumes.title')}>
  <header>
    <h2>{t('volumes.title')}</h2>
    <button
      class="refresh"
      type="button"
      onclick={() => session.loadVolumes()}
      title={t('volumes.refresh')}
      aria-label={t('volumes.refresh')}
    >
      ⟳
    </button>
  </header>

  {#if session.volumesLoading && volumes.length === 0}
    <p class="hint">{t('common.loading')}</p>
  {:else if session.volumesError}
    <div class="error">
      <p>{t('volumes.error')}</p>
      <button type="button" onclick={() => session.loadVolumes()}>{t('volumes.retry')}</button>
    </div>
  {:else if volumes.length === 0}
    <p class="hint">{t('volumes.empty')}</p>
    <p class="hint small">{t('volumes.empty_hint')}</p>
  {:else}
    <ul>
      {#each volumes as volume (volume.id)}
        {@const used = volume.totalBytes > 0 ? volume.usedBytes / volume.totalBytes : 0}
        <li class:selected={session.selectedVolume === volume.id}>
          <button
            type="button"
            class="card"
            onclick={() => (session.selectedVolume = volume.id)}
            ondblclick={() => scan(volume.mountPoint)}
            aria-pressed={session.selectedVolume === volume.id}
          >
            <div class="row">
              <span class="label" title={volume.mountPoint}>{volume.label}</span>
              <span class="kind">{kindLabel(volume.kind)}</span>
            </div>
            <div class="meta">
              {formatBytes(volume.availableBytes, { units, locale })} / {formatBytes(volume.totalBytes, { units, locale })}
            </div>
            <ProgressBar value={used} tone={tone(used)} label={formatPercent(used, locale)} />
            <div class="foot">
              <span>{volume.filesystem || t('common.unknown')}</span>
              <span>{formatPercent(used, locale)}</span>
            </div>
          </button>
        </li>
      {/each}
    </ul>
  {/if}

  {#if session.selectedVolume}
    <div class="footer">
      <button class="scan" type="button" onclick={() => scan(session.selectedVolume ?? '')}>
        {t('scan.start')}
      </button>
    </div>
  {/if}
</aside>

<style>
  .volumes {
    display: flex;
    flex-direction: column;
    width: 264px;
    min-width: 264px;
    border-right: 1px solid var(--fs-border);
    background: var(--fs-surface);
    overflow: hidden;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--fs-space-3) var(--fs-space-3) var(--fs-space-2);
  }

  h2 {
    margin: 0;
    font-size: var(--fs-text-sm);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--fs-text-muted);
  }

  .refresh {
    border: 0;
    background: none;
    color: var(--fs-text-muted);
    font-size: 15px;
    cursor: pointer;
    padding: 2px 6px;
    border-radius: var(--fs-radius-sm);
  }

  .refresh:hover {
    background: var(--fs-surface-2);
    color: var(--fs-text);
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0 var(--fs-space-2);
    overflow-y: auto;
    flex: 1;
  }

  li {
    margin-bottom: var(--fs-space-1);
  }

  .card {
    display: flex;
    flex-direction: column;
    gap: var(--fs-space-1);
    width: 100%;
    padding: var(--fs-space-2) var(--fs-space-3);
    border: 1px solid transparent;
    border-radius: var(--fs-radius);
    background: transparent;
    text-align: left;
    cursor: pointer;
  }

  .card:hover {
    background: var(--fs-surface-2);
  }

  li.selected .card {
    border-color: var(--fs-accent);
    background: var(--fs-accent-soft);
  }

  .row {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: var(--fs-space-2);
  }

  .label {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .kind {
    font-size: var(--fs-text-xs);
    color: var(--fs-text-faint);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .meta {
    font-size: var(--fs-text-sm);
    color: var(--fs-text-muted);
  }

  .foot {
    display: flex;
    justify-content: space-between;
    font-size: var(--fs-text-xs);
    color: var(--fs-text-faint);
  }

  .hint {
    margin: var(--fs-space-2) var(--fs-space-3);
    color: var(--fs-text-muted);
    font-size: var(--fs-text-sm);
  }

  .hint.small {
    color: var(--fs-text-faint);
    font-size: var(--fs-text-xs);
  }

  .error {
    padding: var(--fs-space-3);
    font-size: var(--fs-text-sm);
    color: var(--fs-danger);
  }

  .error button {
    margin-top: var(--fs-space-2);
    border: 1px solid var(--fs-border);
    background: var(--fs-surface);
    border-radius: var(--fs-radius-sm);
    padding: 2px 8px;
    cursor: pointer;
  }

  .footer {
    padding: var(--fs-space-3);
    border-top: 1px solid var(--fs-border);
  }

  .scan {
    width: 100%;
    height: 32px;
    border: 1px solid var(--fs-accent);
    border-radius: var(--fs-radius);
    background: var(--fs-accent);
    color: var(--fs-text-inverse);
    font-weight: 600;
    cursor: pointer;
  }

  .scan:hover {
    background: var(--fs-accent-hover);
  }
</style>
