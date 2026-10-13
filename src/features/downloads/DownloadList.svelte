<script lang="ts">
  import Icon from '../../lib/icons/Icon.svelte';
  import { backend } from '../../lib/tauri/store.svelte';
  import { api } from '../../lib/tauri/api';
  import { bytes, date, percentage, statusLabels } from '../../lib/format';
  import type { Download } from '../../lib/types';
  let { items, compact = false }: { items: Download[]; compact?: boolean } =
    $props();
</script>

<div class="download-list" class:compact>
  {#each items as original (original.id)}
    {@const item = backend.row(original)}
    {@const percent = percentage(item.downloadedBytes, item.totalBytes)}
    <article class="download-row">
      <div class="file-icon" class:video={item.mediaType === 'video'}>
        <Icon name={item.mediaType} size={23} />
      </div>
      <div class="file-info">
        <div class="file-title" title={item.filename}>{item.filename}</div>
        <div class="file-meta">
          {bytes(item.totalBytes)} <span>·</span>
          {date(item.completedAt ?? item.startedAt ?? item.createdAt)}
          {#if item.mediaGroupId}<span>· Альбом</span>{/if}
        </div>
        {#if item.status === 'downloading' || item.status === 'finalizing' || compact}
          <div class="progress-details">
            <span
              >{bytes(item.downloadedBytes)}{#if item.totalBytes != null}
                / {bytes(item.totalBytes)}{/if}</span
            ><span
              >{item.speed > 0
                ? `${bytes(item.speed)}/с`
                : item.status === 'completed'
                  ? 'Сохранён'
                  : item.status === 'failed'
                    ? 'Ошибка'
                    : item.status === 'cancelled'
                      ? 'Отменён'
                      : item.status === 'queued'
                        ? 'В очереди'
                        : item.error
                          ? 'Ожидание повтора'
                          : 'Получение файла…'}</span
            >
          </div>
          <div
            class="progress-track"
            data-state={item.status === 'failed'
              ? 'error'
              : item.error ||
                  item.status === 'cancelled' ||
                  item.status === 'queued'
                ? 'warning'
                : 'success'}
            role="progressbar"
            aria-label={`Загрузка ${item.filename}`}
            aria-valuemin="0"
            aria-valuemax="100"
            aria-valuenow={item.status === 'completed'
              ? 100
              : (percent ?? undefined)}
          >
            <div
              class:indeterminate={percent === null &&
                item.status === 'downloading' &&
                !item.error}
              style:width={['completed', 'failed', 'cancelled'].includes(
                item.status,
              ) || item.error
                ? '100%'
                : percent === null
                  ? '35%'
                  : `${percent}%`}
            ></div>
          </div>
        {/if}
        {#if item.error && item.status !== 'completed'}<p class="inline-error">
            {item.error}
          </p>{/if}
      </div>
      <span class="badge" data-status={item.status}
        >{#if item.status === 'completed'}<Icon
            name="success"
            size={14}
          />{/if}{statusLabels[item.status]}</span
      >
      {#if !compact}
        <div class="row-actions">
          {#if item.status === 'completed'}
            <button
              class="icon-button"
              title="Открыть файл"
              aria-label={`Открыть ${item.filename}`}
              disabled={!!backend.busy}
              onclick={() =>
                backend.action('open', () => api.openDownload(item.id))}
              ><Icon name={item.mediaType} size={18} /></button
            >
            <button
              class="icon-button"
              title="Показать в Проводнике"
              aria-label={`Показать ${item.filename} в Проводнике`}
              disabled={!!backend.busy}
              onclick={() =>
                backend.action('reveal', () => api.openDownload(item.id, true))}
              ><Icon name="folderOpen" size={18} /></button
            >
          {:else if item.status === 'failed' || item.status === 'cancelled'}
            <button
              class="icon-button"
              title="Повторить загрузку"
              aria-label={`Повторить ${item.filename}`}
              disabled={!!backend.busy}
              onclick={() => backend.action('retry', () => api.retry(item.id))}
              ><Icon name="refresh" size={18} /></button
            >
          {:else if item.status !== 'finalizing'}
            <button
              class="icon-button"
              title="Отменить загрузку"
              aria-label={`Отменить ${item.filename}`}
              disabled={!!backend.busy}
              onclick={() =>
                backend.action('cancel', () => api.cancel(item.id))}
              ><Icon name="cancel" size={18} /></button
            >
          {/if}
        </div>
      {/if}
    </article>
  {/each}
</div>
