import { describe, it, expect } from 'vitest';
import { reconcileProgress } from './progress';
import type { Download } from '../types';

function row(status: Download['status'], downloadedBytes = 0): Download {
  return { id: 1, status, downloadedBytes } as Download;
}
describe('backend event reconciliation', () => {
  it('retains real streamed bytes newer than persistent metadata', () => {
    expect(
      reconcileProgress([row('downloading', 0)], {
        1: row('downloading', 250),
      })[1].downloadedBytes,
    ).toBe(250);
  });
  it('never revives completed or recovered queued tasks', () => {
    expect(
      reconcileProgress([row('completed')], { 1: row('downloading') }),
    ).toEqual({});
    expect(
      reconcileProgress([row('queued')], { 1: row('downloading') }),
    ).toEqual({});
  });
  it('retains terminal events until SQLite has caught up', () => {
    expect(
      reconcileProgress([row('downloading')], { 1: row('completed') })[1]
        .status,
    ).toBe('completed');
    expect(
      reconcileProgress([row('completed')], { 1: row('completed') }),
    ).toEqual({});
  });
});
