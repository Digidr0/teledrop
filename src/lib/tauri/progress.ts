import type { Download } from '../types';

/** Keep streamed bytes newer than SQLite, but never revive a terminal task. */
export function reconcileProgress(
  rows: Download[],
  streamed: Record<number, Download>,
): Record<number, Download> {
  const persisted = new Map(rows.map((row) => [row.id, row]));
  const next: Record<number, Download> = {};
  for (const [id, progress] of Object.entries(streamed)) {
    const saved = persisted.get(Number(id));
    const active = ['downloading', 'finalizing'].includes(progress.status);
    if (
      active &&
      (!saved || ['downloading', 'finalizing'].includes(saved.status))
    )
      next[Number(id)] = progress;
    // A terminal event may arrive during an in-flight stale snapshot request.
    if (
      !active &&
      saved &&
      saved.status !== progress.status &&
      ['downloading', 'finalizing'].includes(saved.status)
    )
      next[Number(id)] = progress;
  }
  return next;
}
