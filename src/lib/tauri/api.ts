import { invoke, isTauri } from '@tauri-apps/api/core';
import type { Snapshot, Settings, HistoryQuery, HistoryPage } from '../types';

function command<T>(name: string, args?: Record<string, unknown>): Promise<T> {
  if (!isTauri())
    return Promise.reject(
      'Откройте TeleDrop как настольное приложение. В браузере backend недоступен.',
    );
  return invoke<T>(name, args);
}
export const api = {
  checkUpdate: () => command<string | null>('check_update'),
  openUpdate: () => command<void>('open_update'),
  checkTelegramApi: () => command<number>('check_telegram_api'),
  snapshot: () => command<Snapshot>('snapshot'),
  history: (query: HistoryQuery) => command<HistoryPage>('history', { query }),
  connect: () => command<void>('connect'),
  disconnect: () => command<void>('disconnect'),
  saveToken: (token: string) =>
    command<void>('validate_and_save_token', { token }),
  removeToken: () => command<void>('remove_token'),
  saveSettings: (settings: Settings) =>
    command<void>('save_settings', { settings }),
  browseFolder: () => command<string | null>('browse_folder'),
  openDestination: () => command<void>('open_destination'),
  openDownload: (id: number, reveal = false) =>
    command<void>('open_download', { id, reveal }),
  cancel: (id: number) => command<void>('cancel_download', { id }),
  retry: (id: number) => command<void>('retry_download', { id }),
  openLogs: () => command<void>('open_logs'),
};
