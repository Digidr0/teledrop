import { describe, it, expect } from 'vitest';
import { resolveTheme } from './index';
import { percentage, bytes } from '../format';
describe('themes and real progress', () => {
  it('follows system only in system mode', () => {
    expect(resolveTheme('system', true)).toBe('dark');
    expect(resolveTheme('system', false)).toBe('light');
    expect(resolveTheme('light', true)).toBe('light');
    expect(resolveTheme('dark', false)).toBe('dark');
  });
  it('does not invent percentages or sizes', () => {
    expect(percentage(10, null)).toBeNull();
    expect(percentage(10, 0)).toBeNull();
    expect(percentage(50, 100)).toBe(50);
    expect(percentage(200, 100)).toBe(100);
    expect(bytes(null)).toBe('Размер неизвестен');
  });
});
