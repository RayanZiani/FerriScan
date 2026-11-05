<script lang="ts">
  import { onMount } from 'svelte';
  import { t } from '$lib/i18n';
  import VolumeList from '$lib/components/shell/VolumeList.svelte';
  import Toolbar from '$lib/components/shell/Toolbar.svelte';
  import StatusBar from '$lib/components/shell/StatusBar.svelte';
  import Overview from '$lib/components/shell/Overview.svelte';
  import TreeTable from '$lib/components/views/TreeTable.svelte';
  import TreemapView from '$lib/components/views/TreemapView.svelte';
  import ViewStub from '$lib/components/views/ViewStub.svelte';
  import Toasts from '$lib/components/common/Toasts.svelte';
  import EmptyState from '$lib/components/common/EmptyState.svelte';
  import { navigation } from '$lib/state/navigation.svelte';
  import { session } from '$lib/state/session.svelte';
  import { settings } from '$lib/state/settings.svelte';

  let ready = $state(false);
  let bootError = $state<string | null>(null);

  onMount(() => {
    let dispose: (() => void) | undefined;
    const unwatch = settings.watchOs();

    // Each step is independent: a failed event subscription must not stop the
    // volume list from loading, and vice versa. The shell always becomes usable.
    void (async () => {
      const results = await Promise.allSettled([
        settings.load(),
        session.subscribe().then((fn) => {
          dispose = fn;
        }),
      ]);
      const failed = results.find((result) => result.status === 'rejected');
      if (failed?.status === 'rejected') {
        bootError = String(failed.reason);
      }
      await session.loadVolumes();
      ready = true;
    })();

    return () => {
      dispose?.();
      unwatch();
    };
  });

  // Jump to the snapshot root once per new snapshot, never on later navigation.
  let seededFor = $state<string | null>(null);
  $effect(() => {
    const meta = session.meta;
    if (!meta || seededFor === meta.id) return;
    seededFor = meta.id;
    void navigation.reset(meta.rootNodeId);
  });
</script>

<div class="shell">
  <header class="titlebar">
    <div class="brand">
      <span class="mark" aria-hidden="true"></span>
      <span class="name">{t('app.name')}</span>
    </div>
    <span class="tagline">{t('app.tagline')}</span>
  </header>

  <div class="body">
    <VolumeList />

    <main class="content">
      {#if bootError}
        <p class="boot-error" role="status">{bootError}</p>
      {/if}
      {#if !ready}
        <p class="loading">{t('common.loading')}</p>
      {:else if !session.meta}
        <EmptyState
          title={t('scan.empty')}
          description={t('scan.empty_hint')}
          actionLabel={session.selectedVolume ? t('scan.start') : undefined}
          onaction={() => void session.scan(session.selectedVolume ?? '')}
        />
      {:else}
        <Overview />
        <Toolbar />
        {#if navigation.view === 'table'}
          <TreeTable />
        {:else if navigation.view === 'treemap'}
          <TreemapView />
        {:else}
          <ViewStub mode={navigation.view} />
        {/if}
      {/if}
    </main>
  </div>

  <StatusBar />
  <Toasts />
</div>

<style>
  .shell {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--fs-bg);
  }

  .titlebar {
    display: flex;
    align-items: baseline;
    gap: var(--fs-space-3);
    height: 44px;
    padding: 0 var(--fs-space-4);
    border-bottom: 1px solid var(--fs-border);
    background: var(--fs-surface);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: var(--fs-space-2);
  }

  .mark {
    width: 16px;
    height: 16px;
    border-radius: 5px;
    background: conic-gradient(
      from 220deg,
      var(--fs-accent) 0deg 80deg,
      var(--fs-surface-3) 80deg 360deg
    );
    border: 1px solid var(--fs-accent);
  }

  .name {
    font-weight: 600;
    font-size: var(--fs-text-lg);
    letter-spacing: -0.01em;
  }

  .tagline {
    font-size: var(--fs-text-xs);
    color: var(--fs-text-faint);
  }

  .body {
    display: flex;
    flex: 1;
    min-height: 0;
  }

  .content {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
  }

  .loading {
    margin: auto;
    color: var(--fs-text-muted);
    font-size: var(--fs-text-sm);
  }

  .boot-error {
    margin: var(--fs-space-2) var(--fs-space-4) 0;
    padding: var(--fs-space-2) var(--fs-space-3);
    border: 1px solid var(--fs-danger);
    border-radius: var(--fs-radius);
    background: var(--fs-danger-soft);
    color: var(--fs-danger);
    font-size: var(--fs-text-sm);
  }
</style>
