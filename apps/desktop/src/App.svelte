<script lang="ts">
  import { onMount } from 'svelte';
  import AppShell from '$lib/components/shell/AppShell.svelte';

  onMount(() => {
    document.documentElement.dataset.ready = 'true';
  });
</script>

<AppShell />

<style>
  :global(#app) {
    opacity: 0;
    transition: opacity var(--fs-duration) var(--fs-ease);
  }

  :global(html[data-ready='true'] #app) {
    opacity: 1;
  }
</style>
