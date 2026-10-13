import { test, expect, type Page } from '@playwright/test';
import type { Snapshot, Download, Theme } from '../../src/lib/types';

// These fixtures live ONLY in the test runner. Production has no fake backend.
function fixture(theme: Theme): Snapshot {
  const statuses: Download['status'][] = [
    'downloading',
    'completed',
    'failed',
    'queued',
    'cancelled',
    'finalizing',
  ];
  return {
    settings: {
      destination: 'C:\\Media\\TeleDrop',
      separateFolders: true,
      concurrency: 2,
      theme,
      notifications: false,
      skipDuplicates: true,
      autoConnect: false,
      allowedChatIds: [],
    },
    connection: {
      running: true,
      state: 'connected',
      message: 'Telegram подключён. Ожидаем фото и видео.',
      botUsername: 'test_fixture_bot',
      botId: 123,
      lastSuccess: 1802346000,
    },
    counts: {
      queued: 1,
      downloading: 2,
      completed: 1,
      failed: 1,
      cancelled: 1,
    },
    downloads: statuses.map((status, id) => ({
      id: id + 1,
      botId: 123,
      updateId: id,
      chatId: 7,
      messageId: id,
      mediaGroupId: id < 2 ? 'test-album' : null,
      filename: `Тестовый файл ${id + 1}.${id % 2 ? 'jpg' : 'mp4'}`,
      mediaType: id % 2 ? 'photo' : 'video',
      totalBytes: id === 0 ? null : 1024 * 1024,
      downloadedBytes: status === 'completed' ? 1024 * 1024 : 256 * 1024,
      speed: 128 * 1024,
      status,
      destination: 'C:\\Media\\TeleDrop',
      finalPath:
        status === 'completed' ? 'C:\\Media\\TeleDrop\\test.jpg' : null,
      error:
        status === 'failed'
          ? 'Нет разрешения на запись в целевую папку.'
          : null,
      attempts: 1,
      createdAt: 1802346000,
      startedAt: 1802346001,
      completedAt: status === 'completed' ? 1802346002 : null,
    })),
    activity: [
      {
        id: 1,
        time: 1802346000,
        level: 'info',
        message: 'Telegram подключён. Ожидаем фото и видео.',
      },
    ],
    tokenConfigured: true,
    version: '0.1.0',
  };
}

async function bridge(page: Page, snapshot: Snapshot) {
  await page.addInitScript((initial) => {
    let next = 0;
    const callbacks = new Map<number, (value: unknown) => void>();
    const listeners = new Map<string, number>();
    const commands: string[] = [];
    const state = structuredClone(initial);
    const preferences = sessionStorage.getItem('test-preferences');
    if (preferences) state.settings = JSON.parse(preferences);
    Object.assign(window, {
      isTauri: true,
      __testCommands: commands,
      __testEmit: (event: string, payload: unknown) =>
        callbacks.get(listeners.get(event) ?? -1)?.({ event, payload, id: 0 }),
      __TAURI_EVENT_PLUGIN_INTERNALS__: { unregisterListener: () => {} },
      __TAURI_INTERNALS__: {
        transformCallback: (fn: (value: unknown) => void) => {
          callbacks.set(++next, fn);
          return next;
        },
        invoke: async (command: string, args: Record<string, unknown>) => {
          commands.push(command);
          if (command === 'plugin:event|listen') {
            listeners.set(args.event as string, args.handler as number);
            return args.handler;
          }
          if (command === 'plugin:event|unlisten') return;
          if (command === 'snapshot') return structuredClone(state);
          if (command === 'history') {
            const query = args.query as {
              search: string;
              status: string;
              mediaType: string;
            };
            const items = state.downloads.filter(
              (d) =>
                d.filename.toLowerCase().includes(query.search.toLowerCase()) &&
                (!query.status || d.status === query.status) &&
                (!query.mediaType || d.mediaType === query.mediaType),
            );
            return { items: structuredClone(items), total: items.length };
          }
          if (command === 'save_settings') {
            state.settings = structuredClone(
              args.settings as Snapshot['settings'],
            );
            sessionStorage.setItem(
              'test-preferences',
              JSON.stringify(state.settings),
            );
            return;
          }
          if (command === 'validate_and_save_token') {
            state.tokenConfigured = true;
            return;
          }
          if (command === 'remove_token') {
            state.tokenConfigured = false;
            return;
          }
          if (command === 'browse_folder') return 'C:\\Test destination';
          if (command === 'disconnect') {
            state.connection.running = false;
            state.connection.state = 'stopped';
            return;
          }
          if (command === 'connect') {
            state.connection.running = true;
            state.connection.state = 'connecting';
            return;
          }
          if (
            [
              'open_download',
              'open_destination',
              'retry_download',
              'cancel_download',
              'open_logs',
            ].includes(command)
          )
            return;
          throw new Error(`Unexpected test command: ${command}`);
        },
      },
    });
  }, snapshot);
}

for (const theme of ['light', 'dark'] as const) {
  test(`all pages, icons and real states — ${theme}`, async ({
    page,
  }, info) => {
    const errors: string[] = [];
    page.on('pageerror', (error) => errors.push(error.message));
    await bridge(page, fixture(theme));
    await page.goto('/');
    await expect(
      page.getByRole('heading', { name: 'Обзор', exact: true }),
    ).toBeVisible();
    await expect(page.locator('html')).toHaveAttribute('data-theme', theme);
    for (const [button, heading] of [
      ['Обзор', 'Обзор'],
      ['Загрузки', 'Загрузки'],
      ['История', 'История'],
      ['Настройки', 'Настройки'],
      ['Журнал', 'Журнал событий'],
    ]) {
      await page
        .locator('aside')
        .getByRole('button', { name: button, exact: true })
        .click();
      await expect(
        page.getByRole('heading', { name: heading, exact: true }),
      ).toBeVisible();
      if (button === 'История')
        await expect(page.getByText('Читаем историю…')).not.toBeVisible();
      expect(await page.locator('svg').count()).toBeGreaterThan(5);
      expect(
        await page.evaluate(
          () => document.documentElement.scrollWidth <= window.innerWidth,
        ),
      ).toBe(true);
      await page.screenshot({
        path: info.outputPath(`${theme}-${button}.png`),
        fullPage: true,
      });
    }
    await page
      .locator('aside')
      .getByRole('button', { name: 'Загрузки', exact: true })
      .click();
    const unknown = page.getByRole('progressbar', {
      name: 'Загрузка Тестовый файл 1.mp4',
    });
    await expect(unknown).not.toHaveAttribute('aria-valuenow');
    await page.evaluate(() => {
      (
        window as unknown as {
          __testEmit: (event: string, payload: unknown) => void;
        }
      ).__testEmit('download-progress', {
        id: 1,
        filename: 'Тестовый файл 1.mp4',
        mediaType: 'video',
        status: 'downloading',
        totalBytes: 1000,
        downloadedBytes: 500,
        speed: 100,
        createdAt: 1802346000,
      });
    });
    await expect(unknown).toHaveAttribute('aria-valuenow', '50');
    expect(errors).toEqual([]);
  });
}

test('settings persist theme, and secret input clears', async ({ page }) => {
  await bridge(page, fixture('system'));
  await page.emulateMedia({ colorScheme: 'dark' });
  await page.goto('/');
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'dark');
  await page
    .locator('aside')
    .getByRole('button', { name: 'Настройки', exact: true })
    .click();
  await page.getByLabel('Тема оформления').selectOption('light');
  await page.getByRole('button', { name: 'Сохранить', exact: true }).click();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  await page.reload();
  await expect(page.locator('html')).toHaveAttribute('data-theme', 'light');
  await page
    .locator('aside')
    .getByRole('button', { name: 'Настройки', exact: true })
    .click();
  await page.getByLabel('Новый токен Telegram бота').fill('123:TEST_NOT_REAL');
  await page.getByRole('button', { name: 'Проверить и сохранить' }).click();
  await expect(page.getByLabel('Новый токен Telegram бота')).toHaveValue('');
  expect(
    await page.evaluate(() =>
      JSON.stringify({ local: localStorage, session: sessionStorage }),
    ),
  ).not.toContain('TEST_NOT_REAL');
});

test('empty states, search, keyboard focus and minimum window', async ({
  page,
}, info) => {
  const state = fixture('light');
  state.downloads = [];
  state.tokenConfigured = false;
  state.connection = {
    running: false,
    state: 'not_configured',
    message: 'Добавьте токен бота в настройках.',
    botId: null,
    botUsername: null,
    lastSuccess: null,
  };
  state.counts = {
    queued: 0,
    downloading: 0,
    completed: 0,
    failed: 0,
    cancelled: 0,
  };
  await bridge(page, state);
  await page.setViewportSize({ width: 840, height: 600 });
  await page.goto('/');
  await expect(
    page.getByRole('button', { name: 'Настроить бота' }),
  ).toBeVisible();
  await page
    .locator('aside')
    .getByRole('button', { name: 'История', exact: true })
    .click();
  await expect(
    page.getByRole('heading', { name: 'История пока пуста' }),
  ).toBeVisible();
  await page.getByLabel('Поиск по имени файла').fill('no match');
  await expect(
    page.getByRole('heading', { name: 'Ничего не найдено' }),
  ).toBeVisible();
  await page.keyboard.press('Tab');
  expect(await page.evaluate(() => document.activeElement?.tagName)).toBe(
    'SELECT',
  );
  expect(
    await page.evaluate(
      () => document.documentElement.scrollWidth <= window.innerWidth,
    ),
  ).toBe(true);
  await page.screenshot({ path: info.outputPath('minimum-window.png') });
});

test('browser without backend never pretends to connect', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('alert')).toContainText('backend недоступен');
  await expect(
    page.getByRole('heading', { name: 'Запустите приложение через Tauri' }),
  ).toBeVisible();
});
