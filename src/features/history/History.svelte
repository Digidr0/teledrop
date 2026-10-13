<script lang="ts">
  import { onDestroy } from 'svelte';
  import { api } from '../../lib/tauri/api';
  import { backend } from '../../lib/tauri/store.svelte';
  import DownloadList from '../downloads/DownloadList.svelte';
  import EmptyState from '../../lib/components/EmptyState.svelte';
  import Icon from '../../lib/icons/Icon.svelte';
  import type { HistoryPage } from '../../lib/types';
  let search = $state('');
  let status = $state('');
  let mediaType = $state('');
  let page = $state(0);
  let result = $state<HistoryPage>({ items: [], total: 0 });
  let loading = $state(true);
  let sequence = 0;
  $effect(() => {
    const query = { search, status, mediaType, page };
    void backend.revision;
    const current = ++sequence;
    loading = true;
    const timer = setTimeout(async () => {
      try {
        const next = await api.history(query);
        if (current === sequence) result = next;
      } catch (error) {
        if (current === sequence) backend.error(error);
      } finally {
        if (current === sequence) loading = false;
      }
    }, 200);
    return () => clearTimeout(timer);
  });
  onDestroy(() => sequence++);
</script>

<header class="page-header">
  <div>
    <h1>История</h1>
    <p>Все полученные файлы. История сохраняется после перезапуска.</p>
  </div>
</header>
<div class="toolbar history-toolbar">
  <label class="search-field"
    ><Icon name="search" size={18} /><input
      aria-label="Поиск по имени файла"
      placeholder="Поиск по имени файла"
      bind:value={search}
      oninput={() => (page = 0)}
    /></label
  >
  <select
    aria-label="Фильтр по статусу"
    bind:value={status}
    onchange={() => (page = 0)}
    ><option value="">Все статусы</option><option value="completed"
      >Сохранено</option
    ><option value="failed">Ошибка</option><option value="queued"
      >В очереди</option
    ><option value="downloading">Загрузка</option><option value="cancelled"
      >Отменено</option
    ></select
  >
  <select
    aria-label="Тип файла"
    bind:value={mediaType}
    onchange={() => (page = 0)}
    ><option value="">Все файлы</option><option value="photo">Фото</option
    ><option value="video">Видео</option></select
  >
</div>
<section class="panel" aria-busy={loading}>
  {#if loading}<p class="panel-note">
      Читаем историю…
    </p>{:else if result.items.length}<DownloadList
      items={result.items}
    />{:else}<EmptyState
      title={search || status || mediaType
        ? 'Ничего не найдено'
        : 'История пока пуста'}
      description={search || status || mediaType
        ? 'Измените запрос или сбросьте фильтры.'
        : 'Полученные фото и видео появятся здесь.'}
      icon="search"
    />{/if}
</section>
<div class="pagination">
  <span class="muted">Всего: {result.total}</span>
  <div>
    <button disabled={page === 0 || loading} onclick={() => page--}
      >Назад</button
    ><span>Страница {page + 1}</span><button
      disabled={(page + 1) * 50 >= result.total || loading}
      onclick={() => page++}>Далее</button
    >
  </div>
</div>
