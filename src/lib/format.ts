import type { DownloadStatus, ConnectionState } from './types';
export function bytes(value: number | null | undefined): string {
  if (value == null) return 'Размер неизвестен';
  if (value < 1024) return `${value} B`;
  if (value < 1024 * 1024) return `${(value / 1024).toFixed(1)} KB`;
  return `${(value / 1024 / 1024).toFixed(1)} MB`;
}
export function date(value: number | null): string {
  return value == null
    ? '—'
    : new Date(value * 1000).toLocaleString('ru-RU', {
        day: '2-digit',
        month: 'short',
        hour: '2-digit',
        minute: '2-digit',
      });
}
export function percentage(
  received: number,
  total: number | null,
): number | null {
  return total != null && total > 0
    ? Math.min(100, Math.max(0, (received / total) * 100))
    : null;
}
export const statusLabels: Record<DownloadStatus, string> = {
  queued: 'В очереди',
  downloading: 'Загрузка',
  finalizing: 'Сохранение',
  completed: 'Сохранено',
  failed: 'Ошибка',
  cancelled: 'Отменено',
};
export const connectionLabels: Record<ConnectionState, string> = {
  not_configured: 'Не настроен',
  connecting: 'Подключение',
  connected: 'Подключён',
  reconnecting: 'Переподключение',
  authentication_error: 'Ошибка токена',
  network_error: 'Ошибка соединения',
  stopped: 'Остановлен',
};
