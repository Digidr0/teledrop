<script lang="ts">
  import Icon from '../../lib/icons/Icon.svelte';
  import EmptyState from '../../lib/components/EmptyState.svelte';
  import DownloadList from '../downloads/DownloadList.svelte';
  import { backend } from '../../lib/tauri/store.svelte';
  import { api } from '../../lib/tauri/api';
  import { connectionLabels } from '../../lib/format';
  import { onMount } from 'svelte';
  import type { Page } from '../../lib/types';
  let { navigate }: { navigate: (page: Page) => void } = $props();
  const snapshot = $derived(backend.snapshot!);
  const connection = $derived(snapshot.connection);
  const running = $derived(connection.running);
  let apiStatus = $state('Не проверено');
  let apiChecking = $state(false);
  let apiAvailable = $state<boolean | null>(null);
  async function checkApi() {
    if (apiChecking) return;
    apiChecking = true;
    apiStatus = 'Проверяем api.telegram.org…';
    try {
      const latency = await api.checkTelegramApi();
      apiAvailable = true;
      apiStatus = `API доступен · ${latency} мс`;
    } catch {
      apiAvailable = false;
      apiStatus = 'Телеграм API недоступен.';
    } finally {
      apiChecking = false;
    }
  }
  onMount(() => {
    void checkApi();
    const timer = setInterval(() => void checkApi(), 30000);
    return () => clearInterval(timer);
  });
  const active = $derived(
    snapshot.downloads.filter((d) =>
      ['queued', 'downloading', 'finalizing'].includes(backend.row(d).status),
    ),
  );
</script>

<header class="page-header">
  <div>
    <h1>Главная</h1>
    <p>Фото и видео из Telegram — прямо на ваш компьютер.</p>
  </div>
</header>
<div class="stats-strip">
  <div>
    <span>Загружается</span><strong>{snapshot.counts.downloading}</strong>
  </div>
  <div><span>В очереди</span><strong>{snapshot.counts.queued}</strong></div>
  <div><span>Сохранено</span><strong>{snapshot.counts.completed}</strong></div>
  <div>
    <span>С ошибкой</span><strong class:danger={snapshot.counts.failed > 0}
      >{snapshot.counts.failed}</strong
    >
  </div>
</div>
<section class="panel connection-panel">
  <div
    class="connection-symbol"
    class:online={connection.state === 'connected'}
  >
    <Icon name="connection" size={25} />
  </div>
  <div class="grow">
    <div class="section-title">
      Telegram <span class="badge" data-status={connection.state}
        >{connectionLabels[connection.state]}</span
      >
    </div>
    {#if connection.state !== 'connected'}<p>{connection.message}</p>{/if}
    <p aria-live="polite" class:danger={apiAvailable === false}>
      <strong>Соединение с Telegram API:</strong>
      {apiStatus}
    </p>
    <p class="muted">При блокировке проверьте VPN.</p>
    <button class="text-button" disabled={apiChecking} onclick={checkApi}
      >Проверить соединение</button
    >
    {#if connection.botUsername}<span class="muted"
        >@{connection.botUsername}</span
      >{/if}
  </div>
  {#if snapshot.tokenConfigured}
    <button
      class:primary={!running}
      disabled={!!backend.busy || (!snapshot.settings.destination && !running)}
      onclick={() =>
        backend.action('connection', running ? api.disconnect : api.connect)}
      >{#if backend.busy === 'connection'}Подождите…{:else}{running
          ? 'Отключиться'
          : 'Подключиться'}{/if}</button
    >
  {:else}<button class="primary" onclick={() => navigate('settings')}
      >Настроить бота</button
    >{/if}
</section>
<section class="panel overview-list-panel">
  <div class="panel-heading">
    <h2>Текущие загрузки</h2>
    <button class="text-button" onclick={() => navigate('downloads')}
      >Все загрузки</button
    >
  </div>
  <!-- svelte-ignore a11y_no_noninteractive_tabindex (Scrollable region must support keyboard scrolling.) -->
  <div
    class="overview-list-scroll"
    role="region"
    aria-label="Текущие загрузки"
    tabindex="0"
  >
    {#if active.length}<DownloadList items={active} />{:else}<EmptyState
        title={snapshot.tokenConfigured
          ? 'Всё готово к приёму файлов'
          : 'Начните с настройки бота'}
        description={snapshot.tokenConfigured
          ? 'Подключитесь и отправьте боту фото или видео до 20 MB. Здесь появится реальный прогресс загрузки.'
          : 'Создайте бота через @BotFather, добавьте токен и выберите папку сохранения.'}
        icon="download"
      />{/if}
  </div>
</section>
<section class="panel recent-panel overview-list-panel">
  <div class="panel-heading">
    <h2>История</h2>
    <button class="text-button" onclick={() => navigate('history')}
      >Открыть историю</button
    >
  </div>
  <!-- svelte-ignore a11y_no_noninteractive_tabindex (Scrollable region must support keyboard scrolling.) -->
  <div
    class="overview-list-scroll"
    role="region"
    aria-label="История загрузок"
    tabindex="0"
  >
    {#if snapshot.downloads.some( (d) => ['completed', 'failed', 'cancelled'].includes(backend.row(d).status) )}
      <DownloadList
        items={snapshot.downloads.filter((d) =>
          ['completed', 'failed', 'cancelled'].includes(backend.row(d).status),
        )}
        compact
      />
    {:else}<p class="panel-note">
        Здесь появится история загрузок файлов.
      </p>{/if}
  </div>
</section>

<style>
  .overview-list-panel {
    height: 280px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .overview-list-panel .panel-heading {
    flex-shrink: 0;
  }
  .overview-list-scroll {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    scrollbar-gutter: stable;
    overscroll-behavior-y: contain;
  }
</style>
