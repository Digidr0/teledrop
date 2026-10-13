<script lang="ts">
  import Icon from '../icons/Icon.svelte';
  import type { Snippet } from 'svelte';
  let {
    title,
    description,
    icon = 'download',
    children,
  }: {
    title: string;
    description: string;
    icon?: 'download' | 'search' | 'connection' | 'clock';
    children?: Snippet;
  } = $props();
</script>

<div class="empty-state">
  <div class="empty-icon"><Icon name={icon} size={32} /></div>
  <h3>{title}</h3>
  <p>{description}</p>
  {#if children}<div class="empty-actions">{@render children()}</div>{/if}
</div>
