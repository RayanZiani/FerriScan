<script lang="ts">
  import { onDestroy } from 'svelte';
  import { t } from '$lib/i18n';
  import { formatBytes, formatPercent } from '$lib/i18n/format';
  import { ipc, IpcError } from '$lib/ipc/client';
  import type { TreemapDto } from '$lib/ipc/types';
  import { navigation } from '$lib/state/navigation.svelte';
  import { settings } from '$lib/state/settings.svelte';

  let canvas: HTMLCanvasElement | undefined = $state();
  let data = $state<TreemapDto | null>(null);
  let error = $state<string | null>(null);
  let hover = $state<{ index: number; x: number; y: number } | null>(null);
  let width = $state(800);
  let height = $state(500);

  const units = $derived(settings.value.binaryUnits ? 'binary' : 'decimal');
  const locale = $derived(settings.value.locale);

  let observer: ResizeObserver | undefined;

  $effect(() => {
    if (!canvas) return;
    observer?.disconnect();
    observer = new ResizeObserver((entries) => {
      const box = entries[0]?.contentRect;
      if (!box) return;
      width = Math.max(1, Math.floor(box.width));
      height = Math.max(1, Math.floor(box.height));
    });
    observer.observe(canvas);
    return () => observer?.disconnect();
  });

  $effect(() => {
    const node = navigation.current;
    if (!node) {
      data = null;
      return;
    }
    let cancelled = false;
    void (async () => {
      try {
        const result = await ipc.getTreemap({
          nodeId: node.id,
          maxDepth: 6,
          budget: 4000,
          width,
          height,
        });
        if (!cancelled) {
          data = result;
          error = null;
        }
      } catch (err) {
        if (!cancelled) error = err instanceof IpcError ? err.message : String(err);
      }
    })();
    return () => {
      cancelled = true;
    };
  });

  $effect(() => {
    const layer = data;
    const target = canvas;
    if (!target) return;
    const ratio = window.devicePixelRatio || 1;
    target.width = Math.floor(width * ratio);
    target.height = Math.floor(height * ratio);
    const ctx = target.getContext('2d');
    if (!ctx) return;
    ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
    ctx.clearRect(0, 0, width, height);
    if (!layer) return;

    const styles = getComputedStyle(document.documentElement);
    for (let i = 0; i < layer.nodes.length; i++) {
      const x = layer.xs[i] ?? 0;
      const y = layer.ys[i] ?? 0;
      const w = layer.ws[i] ?? 0;
      const h = layer.hs[i] ?? 0;
      if (w < 1 || h < 1) continue;
      const depth = layer.depths[i] ?? 0;
      const hue = `--fs-chart-${((i + depth * 3) % 12) + 1}`;
      ctx.fillStyle = styles.getPropertyValue(hue).trim() || '#4b5563';
      ctx.globalAlpha = Math.max(0.28, 0.85 - depth * 0.12);
      ctx.fillRect(x, y, w, h);
      ctx.globalAlpha = 1;
      if (depth === 0) {
        ctx.strokeStyle = styles.getPropertyValue('--fs-border-strong').trim();
        ctx.lineWidth = 1;
        ctx.strokeRect(x + 0.5, y + 0.5, w - 1, h - 1);
      }
    }
  });

  function onMove(event: MouseEvent): void {
    const layer = data;
    const target = canvas;
    if (!layer || !target) return;
    const rect = target.getBoundingClientRect();
    const x = event.clientX - rect.left;
    const y = event.clientY - rect.top;
    for (let i = layer.nodes.length - 1; i >= 0; i--) {
      const rx = layer.xs[i] ?? 0;
      const ry = layer.ys[i] ?? 0;
      const rw = layer.ws[i] ?? 0;
      const rh = layer.hs[i] ?? 0;
      if (x >= rx && x <= rx + rw && y >= ry && y <= ry + rh) {
        hover = { index: i, x, y };
        return;
      }
    }
    hover = null;
  }

  function onClick(): void {
    const layer = data;
    const index = hover?.index;
    if (!layer || index === undefined) return;
    const id = layer.nodes[index];
    if (id === undefined || id === 0xffffffff) return;
    void navigation.open(id);
  }

  const hoveredNode = $derived(hover && data ? data.nodes[hover.index] : undefined);

  onDestroy(() => observer?.disconnect());
</script>

<div class="treemap">
  {#if !navigation.current}
    <p class="placeholder">{t('scan.empty')}</p>
  {:else if error}
    <p class="placeholder error">{error}</p>
  {:else}
    <canvas bind:this={canvas} onmousemove={onMove} onmouseleave={() => (hover = null)} onclick={onClick}
    ></canvas>
    {#if hover && hoveredNode !== undefined && hoveredNode !== 0xffffffff}
      <div class="tooltip" style="left: {hover.x + 12}px; top: {hover.y + 12}px">
        <strong>{formatBytes(data?.sizes[hover.index] ?? 0, { units, locale })}</strong>
        <span>{formatPercent(
          (data?.sizes[hover.index] ?? 0) / Math.max(data?.sizes[0] ?? 1, 1),
          locale,
        )}</span>
      </div>
    {/if}
  {/if}
</div>

<style>
  .treemap {
    position: relative;
    flex: 1;
    overflow: hidden;
    background: var(--fs-bg);
  }

  canvas {
    display: block;
    width: 100%;
    height: 100%;
    cursor: pointer;
  }

  .tooltip {
    position: absolute;
    pointer-events: none;
    display: flex;
    gap: var(--fs-space-2);
    align-items: baseline;
    padding: 3px var(--fs-space-2);
    border: 1px solid var(--fs-border);
    border-radius: var(--fs-radius-sm);
    background: var(--fs-surface);
    box-shadow: var(--fs-shadow-1);
    font-size: var(--fs-text-xs);
    white-space: nowrap;
  }

  .placeholder {
    padding: var(--fs-space-5);
    text-align: center;
    color: var(--fs-text-muted);
  }

  .placeholder.error {
    color: var(--fs-danger);
  }
</style>
