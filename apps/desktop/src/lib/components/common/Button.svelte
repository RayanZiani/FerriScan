<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    variant?: 'primary' | 'secondary' | 'ghost' | 'danger';
    size?: 'sm' | 'md';
    disabled?: boolean;
    title?: string;
    type?: 'button' | 'submit';
    onclick?: (event: MouseEvent) => void;
    children: Snippet;
  }

  let {
    variant = 'secondary',
    size = 'md',
    disabled = false,
    title,
    type = 'button',
    onclick,
    children,
  }: Props = $props();
</script>

<button
  class="btn {variant} {size}"
  {type}
  {disabled}
  {title}
  aria-disabled={disabled}
  {onclick}
>
  {@render children()}
</button>

<style>
  .btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: var(--fs-space-2);
    border: 1px solid var(--fs-border);
    border-radius: var(--fs-radius);
    background: var(--fs-surface);
    color: var(--fs-text);
    font-weight: 500;
    cursor: pointer;
    white-space: nowrap;
    transition:
      background var(--fs-duration-fast) var(--fs-ease),
      border-color var(--fs-duration-fast) var(--fs-ease);
  }

  .btn.md {
    height: 30px;
    padding: 0 var(--fs-space-3);
  }

  .btn.sm {
    height: 24px;
    padding: 0 var(--fs-space-2);
    font-size: var(--fs-text-sm);
  }

  .btn:hover:not(:disabled) {
    background: var(--fs-surface-2);
  }

  .btn.primary {
    background: var(--fs-accent);
    border-color: var(--fs-accent);
    color: var(--fs-text-inverse);
  }

  .btn.primary:hover:not(:disabled) {
    background: var(--fs-accent-hover);
    border-color: var(--fs-accent-hover);
  }

  .btn.ghost {
    background: transparent;
    border-color: transparent;
  }

  .btn.ghost:hover:not(:disabled) {
    background: var(--fs-surface-2);
  }

  .btn.danger {
    background: var(--fs-danger);
    border-color: var(--fs-danger);
    color: #fff;
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
