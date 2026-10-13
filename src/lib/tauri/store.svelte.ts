import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { isTauri } from '@tauri-apps/api/core';
import { api } from './api';
import type { Connection, Download, Snapshot } from '../types';
import { reconcileProgress } from './progress';

class BackendStore {
  snapshot = $state<Snapshot | null>(null);
  progress = $state<Record<number, Download>>({});
  notice = $state('');
  noticeKind = $state<'error' | 'success'>('error');
  busy = $state('');
  revision = $state(0);
  private unlisten: UnlistenFn[] = [];
  private refreshTimer: ReturnType<typeof setTimeout> | undefined;
  private disposed = false;
  private refreshing = false;
  private pending = false;

  async refresh() {
    if (this.refreshing) {
      this.pending = true;
      return;
    }
    this.refreshing = true;
    try {
      this.snapshot = await api.snapshot();
      this.progress = reconcileProgress(this.snapshot.downloads, this.progress);
      this.revision++;
    } catch (error) {
      this.error(error);
    } finally {
      this.refreshing = false;
      if (this.pending && !this.disposed) {
        this.pending = false;
        await this.refresh();
      }
    }
  }

  scheduleRefresh() {
    if (this.refreshTimer || this.disposed) return;
    this.refreshTimer = setTimeout(() => {
      this.refreshTimer = undefined;
      void this.refresh();
    }, 180);
  }

  async initialize() {
    this.disposed = false;
    if (isTauri()) {
      for (const subscription of [
        listen<Connection>('connection', ({ payload }) => {
          if (this.snapshot) this.snapshot.connection = payload;
        }),
        listen<Download>('download-progress', ({ payload }) => {
          this.progress[payload.id] = payload;
        }),
        listen('data-changed', () => this.scheduleRefresh()),
      ]) {
        const stop = await subscription;
        if (this.disposed) stop();
        else this.unlisten.push(stop);
      }
    }
    if (!this.disposed) await this.refresh();
  }

  destroy() {
    this.disposed = true;
    this.unlisten.forEach((stop) => stop());
    this.unlisten = [];
    clearTimeout(this.refreshTimer);
    this.refreshTimer = undefined;
  }

  error(error: unknown) {
    this.noticeKind = 'error';
    this.notice =
      typeof error === 'string'
        ? error
        : 'Не удалось выполнить действие. Откройте журнал для диагностики.';
  }

  async action(
    name: string,
    fn: () => Promise<unknown>,
    success?: string,
  ): Promise<boolean> {
    if (this.busy) return false;
    this.busy = name;
    this.notice = '';
    try {
      await fn();
      await this.refresh();
      if (success) {
        this.noticeKind = 'success';
        this.notice = success;
      }
      return true;
    } catch (error) {
      this.error(error);
      return false;
    } finally {
      this.busy = '';
    }
  }

  row(download: Download) {
    return this.progress[download.id] ?? download;
  }
}

export const backend = new BackendStore();
