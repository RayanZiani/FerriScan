<script lang="ts">
  import { t } from '$lib/i18n';
  import type { ViewMode } from '$lib/state/navigation.svelte';

  interface Props {
    mode: ViewMode;
  }

  let { mode }: Props = $props();
</script>

<div class="stub">
  <p>{t('common.loading')}</p>
  <span>{mode}</span>
</div>

<style>
  .stub {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--fs-space-2);
    color: var(--fs-text-muted);
    font-size: var(--fs-text-sm);
  }

  span {
    font-size: var(--fs-text-xs);
    color: var(--fs-text-faint);
    text-transform: uppercase;
    letter-spacing: 0.06em;
  }
</style>
