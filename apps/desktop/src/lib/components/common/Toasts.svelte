<script lang="ts">
  import { t } from '$lib/i18n';
  import { toasts } from '$lib/state/toasts.svelte';

  function dismiss(id: number): void {
    toasts.dismiss(id);
  }
</script>

<div class="toasts" aria-live="polite" aria-atomic="false">
  {#each toasts.items as toast (toast.id)}
    <div class="toast {toast.kind}" role="status">
      <span class="message">{toast.message}</span>
      <button class="close" type="button" onclick={() => dismiss(toast.id)} aria-label={t('common.dismiss')}>
        ×
      </button>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    right: var(--fs-space-4);
    bottom: var(--fs-space-4);
    display: flex;
    flex-direction: column;
    gap: var(--fs-space-2);
    z-index: 60;
    max-width: 380px;
  }

  .toast {
    display: flex;
    align-items: flex-start;
    gap: var(--fs-space-2);
    padding: var(--fs-space-2) var(--fs-space-3);
    border: 1px solid var(--fs-border);
    border-left: 3px solid var(--fs-accent);
    border-radius: var(--fs-radius);
    background: var(--fs-surface);
    box-shadow: var(--fs-shadow-2);
    font-size: var(--fs-text-sm);
  }

  .toast.error {
    border-left-color: var(--fs-danger);
  }

  .toast.success {
    border-left-color: var(--fs-success);
  }

  .message {
    flex: 1;
  }

  .close {
    border: 0;
    background: none;
    color: var(--fs-text-muted);
    font-size: 16px;
    line-height: 1;
    cursor: pointer;
    padding: 0 2px;
  }

  .close:hover {
    color: var(--fs-text);
  }
</style>
