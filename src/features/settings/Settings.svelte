<script lang="ts">
  import { backend } from '../../lib/tauri/store.svelte';
  import { api } from '../../lib/tauri/api';
  import Icon from '../../lib/icons/Icon.svelte';
  import type { Settings } from '../../lib/types';
  import { Button } from '../../lib/components/ui/button';
  let draft = $state<Settings>({ ...backend.snapshot!.settings });
  let token = $state('');
  let chats = $state(draft.allowedChatIds.join(', '));
  let confirmRemove = $state(false);
  async function saveToken() {
    const pending = token;
    token = '';
    await backend.action(
      'token',
      () => api.saveToken(pending),
      'Токен проверен через getMe и сохранён в Windows Credential Manager.',
    );
  }
  async function browse() {
    await backend.action('browse', async () => {
      const folder = await api.browseFolder();
      if (folder) draft.destination = folder;
    });
  }
  async function save() {
    const values = chats.trim()
      ? chats.split(',').map((s) => Number(s.trim()))
      : [];
    if (values.some((n) => !Number.isSafeInteger(n) || n === 0)) {
      backend.error('Укажите числовые chat ID через запятую.');
      return;
    }
    draft.allowedChatIds = values;
    await backend.action(
      'settings',
      () => api.saveSettings($state.snapshot(draft)),
      'Настройки сохранены.',
    );
  }
</script>

<header class="page-header">
  <div>
    <h1>Настройки</h1>
    <p>Подключение, сохранение файлов и внешний вид.</p>
  </div>
  <Button class="primary" disabled={!!backend.busy} onclick={save}
    >{backend.busy === 'settings' ? 'Сохранение…' : 'Сохранить'}</Button
  >
</header>
<h2 class="group-heading">Telegram</h2>
<section class="panel settings-panel">
  <div class="setting-row">
    <Icon name="connection" />
    <div class="grow">
      <h3>Токен бота</h3>
      <p>
        {backend.snapshot!.tokenConfigured
          ? 'Токен сохранён в Windows Credential Manager.'
          : 'Создайте бота через @BotFather и вставьте его токен.'}
      </p>
    </div>
    <span
      class="badge"
      data-status={backend.snapshot!.tokenConfigured ? 'completed' : 'stopped'}
      >{backend.snapshot!.tokenConfigured ? 'Настроен' : 'Не настроен'}</span
    >
  </div>
  <form
    class="token-form"
    onsubmit={(event) => {
      event.preventDefault();
      void saveToken();
    }}
  >
    <input
      type="password"
      aria-label="Новый токен Telegram бота"
      autocomplete="off"
      spellcheck="false"
      placeholder="Вставьте токен — он не будет показан повторно"
      bind:value={token}
    /><button type="submit" disabled={!token.trim() || !!backend.busy}
      >{backend.busy === 'token'
        ? 'Проверка…'
        : 'Проверить и сохранить'}</button
    >
  </form>
  {#if backend.snapshot!.tokenConfigured}<div class="token-remove">
      {#if confirmRemove}<span>Удалить токен и остановить получение?</span
        ><button
          class="danger-button"
          disabled={!!backend.busy}
          onclick={async () => {
            await backend.action(
              'remove-token',
              api.removeToken,
              'Токен удалён.',
            );
            confirmRemove = false;
          }}>Удалить</button
        ><button onclick={() => (confirmRemove = false)}>Отмена</button
        >{:else}<button
          class="text-button"
          onclick={() => (confirmRemove = true)}
          >Удалить сохранённый токен</button
        >{/if}
    </div>{/if}
  <p class="panel-note">
    Проверка токена не означает активное соединение. Подключение запускается
    отдельно на странице «Главная».
  </p>
</section>
<h2 class="group-heading">Сохранение файлов</h2>
<section class="panel settings-panel">
  <div class="setting-row">
    <Icon name="folder" />
    <div class="grow">
      <h3>Папка назначения</h3>
      <p class="path" title={draft.destination}>
        {draft.destination || 'Не выбрана'}
      </p>
    </div>
    <button disabled={!!backend.busy} onclick={browse}>Обзор…</button>
  </div>
  <label class="setting-row"
    ><Icon name="photo" /><span class="grow"
      ><strong>Отдельные папки для фото и видео</strong><small
        >Подпапки Photos и Videos внутри выбранной папки.</small
      ></span
    ><input
      type="checkbox"
      class="switch"
      role="switch"
      bind:checked={draft.separateFolders}
    /></label
  >
  <label class="setting-row"
    ><Icon name="download" /><span class="grow"
      ><strong>Одновременные загрузки</strong><small
        >Новые задания используют выбранное значение.</small
      ></span
    ><select bind:value={draft.concurrency}
      >{#each [1, 2, 3, 4, 5, 6] as n (n)}<option value={n}>{n}</option
        >{/each}</select
    ></label
  >
  <label class="setting-row"
    ><Icon name="success" /><span class="grow"
      ><strong>Пропускать повторно присланные файлы</strong><small
        >По file_unique_id в пределах одного бота. Существующие файлы не
        перезаписываются.</small
      ></span
    ><input
      type="checkbox"
      class="switch"
      role="switch"
      bind:checked={draft.skipDuplicates}
    /></label
  >
  <p class="panel-note">
    Смена папки применяется к новым сообщениям. Уже принятые задания сохраняют
    исходный путь.
  </p>
</section>
<h2 class="group-heading">Внешний вид и поведение</h2>
<section class="panel settings-panel">
  <label class="setting-row"
    ><Icon name={draft.theme === 'dark' ? 'moon' : 'sun'} /><span class="grow"
      ><strong>Тема оформления</strong><small
        >Системная тема следует настройкам Windows.</small
      ></span
    ><select bind:value={draft.theme}
      ><option value="system">Как в системе</option><option value="light"
        >Светлая</option
      ><option value="dark">Тёмная</option></select
    ></label
  >
  <label class="setting-row"
    ><Icon name="notification" /><span class="grow"
      ><strong>Уведомления Windows</strong><small
        >Сообщать о завершённых загрузках.</small
      ></span
    ><input
      type="checkbox"
      class="switch"
      role="switch"
      bind:checked={draft.notifications}
    /></label
  >
  <label class="setting-row"
    ><Icon name="connection" /><span class="grow"
      ><strong>Подключаться при запуске приложения</strong><small
        >Не добавляет приложение в автозагрузку Windows.</small
      ></span
    ><input
      type="checkbox"
      class="switch"
      role="switch"
      bind:checked={draft.autoConnect}
    /></label
  >
  <p class="panel-note">
    При закрытии окна TeleDrop завершает работу и сохраняет очередь. Работа в
    трее и запуск с Windows в этой версии не включены.
  </p>
</section>
<details class="panel advanced-panel">
  <summary>Безопасность и диагностика</summary>
  <div class="advanced-content">
    <label
      >Разрешённые chat ID<input
        bind:value={chats}
        placeholder="Например: 123456789, -1001234567890"
      /></label
    >
    <p class="footnote">
      Пустой список разрешает сообщения из всех доступных боту чатов. Ограничьте
      список, если адрес бота известен другим людям. Chat ID полученных
      сообщений доступны в базе истории.
    </p>
    <button
      disabled={!!backend.busy}
      onclick={() => backend.action('logs', api.openLogs)}
      ><Icon name="folderOpen" size={18} />Открыть технические логи</button
    >
  </div>
</details>
<section class="about-panel">
  <div class="brand-small">
    <Icon name="download" size={22} /><strong>TeleDrop</strong><span
      >Версия {backend.snapshot!.version}</span
    >
  </div>
  <p>
    Telegram Bot API · Скачивание до 20 MB · Данные хранятся на этом компьютере.
  </p>
</section>
