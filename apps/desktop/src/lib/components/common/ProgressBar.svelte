<script lang="ts">
  interface Props {
    /** Filled ratio, 0..1. */
    value: number;
    label?: string;
    tone?: 'accent' | 'warning' | 'danger';
  }

  let { value, label, tone = 'accent' }: Props = $props();

  const clamped = $derived(Math.min(Math.max(Number.isFinite(value) ? value : 0, 0), 1));
  const percent = $derived(Math.round(clamped * 100));
</script>

<div
  class="bar {tone}"
  role="progressbar"
  aria-valuemin={0}
  aria-valuemax={100}
  aria-valuenow={percent}
  aria-label={label ?? 'Capacity'}
>
  <div class="fill" style="width: {percent}%"></div>
</div>

<style>
  .bar {
    height: 6px;
    border-radius: 999px;
    background: var(--fs-surface-3);
    overflow: hidden;
  }

  .fill {
    height: 100%;
    border-radius: inherit;
    background: var(--fs-accent);
    transition: width var(--fs-duration) var(--fs-ease);
  }

  .bar.warning .fill {
    background: var(--fs-warning);
  }

  .bar.danger .fill {
    background: var(--fs-danger);
  }

  :global(:root[data-contrast='high']) .bar {
    border: 1px solid currentColor;
  }
</style>
