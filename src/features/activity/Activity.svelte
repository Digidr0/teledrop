<script lang="ts">
  import { backend } from '../../lib/tauri/store.svelte';
  import { api } from '../../lib/tauri/api';
  import { date } from '../../lib/format';
  import Icon from '../../lib/icons/Icon.svelte';
  import DownloadList from '../downloads/DownloadList.svelte';
</script>

<header class="page-header">
  <div>
    <h1>Журнал событий</h1>
    <p>Подключение и диагностика — без токенов и секретных URL.</p>
  </div>
  <button
    disabled={!!backend.busy}
    onclick={() => backend.action('logs', api.openLogs)}
    ><Icon name="folderOpen" size={18} />Файлы логов</button
  >
</header>
<section class="panel">
  <div class="panel-heading"><h2>Загрузка файлов</h2></div>
  {#if backend.snapshot!.downloads.length}
    <DownloadList items={backend.snapshot!.downloads} compact />
  {:else}<p class="panel-note">
      Файлы и их прогресс появятся здесь после получения сообщения.
    </p>{/if}
</section>
<section class="panel">
  <ul class="activity-list full-log">
    {#each backend.snapshot!.activity as entry (entry.id)}<li>
        <Icon name={entry.level === 'error' ? 'error' : 'clock'} size={18} />
        <div class="grow">
          <span>{entry.message}</span><time>{date(entry.time)}</time>
        </div>
        <span
          class="badge"
          data-status={entry.level === 'error' ? 'failed' : 'stopped'}
          >{entry.level === 'error' ? 'Ошибка' : 'Событие'}</span
        >
      </li>{:else}<li>Событий пока нет.</li>{/each}
  </ul>
</section>
<p class="footnote">
  Последние 100 событий. База хранит до 2000 событий, технические логи — до 7
  дневных файлов.
</p>
