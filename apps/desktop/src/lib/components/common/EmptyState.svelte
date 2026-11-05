<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    title: string;
    description?: string;
    actionLabel?: string;
    onaction?: () => void;
    icon?: Snippet;
  }

  let { title, description, actionLabel, onaction, icon }: Props = $props();
</script>

<div class="empty">
  <div class="glyph" aria-hidden="true">
    {#if icon}
      {@render icon()}
    {:else}
      <svg viewBox="0 0 48 48" width="40" height="40" fill="none" stroke="currentColor" stroke-width="2">
        <circle cx="24" cy="24" r="16" opacity="0.35" />
        <path d="M24 8v16l11 7" opacity="0.7" />
      </svg>
    {/if}
  </div>
  <h2>{title}</h2>
  {#if description}
    <p>{description}</p>
  {/if}
  {#if actionLabel && onaction}
    <button type="button" class="action" onclick={onaction}>{actionLabel}</button>
  {/if}
</div>

<style>
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--fs-space-3);
    padding: var(--fs-space-6);
    text-align: center;
    color: var(--fs-text-muted);
    height: 100%;
  }

  .glyph {
    color: var(--fs-text-faint);
  }

  h2 {
    margin: 0;
    font-size: var(--fs-text-lg);
    font-weight: 600;
    color: var(--fs-text);
  }

  p {
    margin: 0;
    max-width: 42ch;
    font-size: var(--fs-text-sm);
  }

  .action {
    margin-top: var(--fs-space-2);
    height: 30px;
    padding: 0 var(--fs-space-4);
    border-radius: var(--fs-radius);
    border: 1px solid var(--fs-accent);
    background: var(--fs-accent);
    color: var(--fs-text-inverse);
    cursor: pointer;
  }
</style>
