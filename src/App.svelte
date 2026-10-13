<script lang="ts">
  import { onMount } from 'svelte';
  import Icon from './lib/icons/Icon.svelte';
  import { backend } from './lib/tauri/store.svelte';
  import { api } from './lib/tauri/api';
  import { applyTheme } from './lib/theme';
  import { connectionLabels } from './lib/format';
  import type { Page } from './lib/types';
  import Overview from './features/overview/Overview.svelte';
  import Downloads from './features/downloads/Downloads.svelte';
  import History from './features/history/History.svelte';
  import Settings from './features/settings/Settings.svelte';
  import Activity from './features/activity/Activity.svelte';
  let page = $state<Page>('overview');
  let updateVersion = $state<string | null>(null);
  let systemDark = $state(
    window.matchMedia('(prefers-color-scheme: dark)').matches,
  );
  let content: HTMLElement;
  const navigation = [
    { id: 'overview' as const, label: 'Главная', icon: 'home' as const },
    { id: 'downloads' as const, label: 'Загрузки', icon: 'download' as const },
    { id: 'history' as const, label: 'История', icon: 'clock' as const },
    { id: 'activity' as const, label: 'Журнал', icon: 'connection' as const },
  ];
  function navigate(next: Page) {
    page = next;
    content?.scrollTo({ top: 0 });
  }
  $effect(() =>
    applyTheme(backend.snapshot?.settings.theme ?? 'system', systemDark),
  );
  onMount(() => {
    const checkUpdate = async () => {
      try {
        updateVersion = await api.checkUpdate();
      } catch {
        /* Offline checks must not interrupt downloads. */
      }
    };
    void checkUpdate();
    const updateTimer = setInterval(
      () => void checkUpdate(),
      6 * 60 * 60 * 1000,
    );
    void backend.initialize().catch((error) => backend.error(error));
    const media = window.matchMedia('(prefers-color-scheme: dark)');
    const changed = () => (systemDark = media.matches);
    media.addEventListener('change', changed);
    return () => {
      clearInterval(updateTimer);
      backend.destroy();
      media.removeEventListener('change', changed);
    };
  });
</script>

<div class="app-shell">
  <aside class="sidebar">
    <div class="brand">
      <div class="brand-mark"><Icon name="telegram" size={22} /></div>
      <div><strong>TeleDrop</strong><span>Telegram → Windows</span></div>
    </div>
    <nav aria-label="Основная навигация">
      {#if updateVersion}<button
          onclick={() => backend.action('update', api.openUpdate)}
          title="Открыть новый релиз на GitHub"
        >
          <Icon name="refresh" /><span>Доступна версия {updateVersion}</span>
        </button>{/if}
      {#each navigation as item (item.id)}<button
          class:active={page === item.id}
          aria-current={page === item.id ? 'page' : undefined}
          aria-label={item.label}
          onclick={() => navigate(item.id)}
          ><Icon name={item.icon} /><span>{item.label}</span
          >{#if item.id === 'downloads' && backend.snapshot && backend.snapshot.counts.queued + backend.snapshot.counts.downloading > 0}<span
              class="nav-count"
              aria-hidden="true"
              >{backend.snapshot.counts.queued +
                backend.snapshot.counts.downloading}</span
            >{/if}</button
        >{/each}
    </nav>
    <div class="sidebar-bottom">
      {#if backend.snapshot}<div class="sidebar-status">
          <span
            class="status-dot"
            class:online={backend.snapshot.connection.state === 'connected'}
          ></span>{connectionLabels[backend.snapshot.connection.state]}
        </div>{/if}<button
        class:active={page === 'settings'}
        aria-current={page === 'settings' ? 'page' : undefined}
        onclick={() => navigate('settings')}
        ><Icon name="settings" /><span>Настройки</span></button
      ><span class="version"
        >TeleDrop {backend.snapshot?.version ?? '1.0.0'}</span
      >
    </div>
  </aside>
  <main bind:this={content} id="main-content">
    {#if backend.notice}<div
        class="notice"
        class:success={backend.noticeKind === 'success'}
        role={backend.noticeKind === 'error' ? 'alert' : 'status'}
      >
        <Icon
          name={backend.noticeKind === 'error' ? 'error' : 'success'}
          size={20}
        /><span class="grow">{backend.notice}</span><button
          class="icon-button"
          aria-label="Закрыть сообщение"
          onclick={() => (backend.notice = '')}
          ><Icon name="cancel" size={18} /></button
        >
      </div>{/if}
    {#if backend.snapshot}
      {#if page === 'overview'}<Overview
          {navigate}
        />{:else if page === 'downloads'}<Downloads
          {navigate}
        />{:else if page === 'history'}<History
        />{:else if page === 'settings'}<Settings />{:else}<Activity />{/if}
    {:else}<header class="page-header">
        <div>
          <h1>TeleDrop</h1>
          <p>
            {backend.notice
              ? 'Для работы нужен настольный backend.'
              : 'Открываем настройки и историю…'}
          </p>
        </div>
      </header>
      <section class="panel startup-panel">
        <Icon name="download" size={36} />
        <h2>
          {backend.notice
            ? 'Запустите приложение через Tauri'
            : 'Подготовка приложения'}
        </h2>
        <p>
          {backend.notice
            ? 'npm run tauri dev — для разработки. Для обычного запуска используйте TeleDrop.exe.'
            : 'Никакие подключения или загрузки пока не подтверждены.'}
        </p>
        <button
          onclick={() => backend.action('refresh', () => backend.refresh())}
          disabled={!!backend.busy}>Проверить backend</button
        >
      </section>{/if}
  </main>
</div>
