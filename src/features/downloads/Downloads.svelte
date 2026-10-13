<script lang="ts">
  import { backend } from '../../lib/tauri/store.svelte';
  import DownloadList from './DownloadList.svelte';
  import EmptyState from '../../lib/components/EmptyState.svelte';
  import type { Page } from '../../lib/types';
  let { navigate }: { navigate: (page: Page) => void } = $props();
  let filter = $state('all');
  const items = $derived(
    backend.snapshot!.downloads.filter(
      (d) =>
        filter === 'all' ||
        (filter === 'active'
          ? ['queued', 'downloading', 'finalizing'].includes(
              backend.row(d).status,
            )
          : backend.row(d).status === filter),
    ),
  );
</script>

<header class="page-header">
  <div>
    <h1>Загрузки</h1>
    <p>Активная очередь и последние полученные файлы.</p>
  </div>
</header>
<div class="toolbar">
  <div class="filter-tabs" aria-label="Фильтр загрузок">
    {#each [['all', 'Все'], ['active', 'Активные'], ['failed', 'С ошибкой']] as [value, label] (value)}<button
        class:chosen={filter === value}
        aria-pressed={filter === value}
        onclick={() => (filter = value)}>{label}</button
      >{/each}
  </div>
  <span class="muted">{items.length} в списке</span>
</div>
<section class="panel">
  {#if items.length}<DownloadList {items} />{:else}<EmptyState
      title={filter === 'all'
        ? 'Пока нет загрузок'
        : 'Загрузок с таким статусом нет'}
      description="Отправьте боту фото или видео. Все сохранённые файлы доступны в истории."
      ><button onclick={() => navigate('overview')}
        >Перейти к подключению</button
      ></EmptyState
    >{/if}
</section>
<p class="footnote">
  Показываются активные задания и последние файлы. При отключении очередь
  сохраняется; незавершённые загрузки начнутся заново.
</p>
