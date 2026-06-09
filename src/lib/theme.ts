// Theme handling: dark / light / system, persisted to localStorage and applied
// via a data-theme attribute on the document root (see app.css).

export type ThemeMode = 'dark' | 'light' | 'system';

const KEY = 'wf.theme';

export function loadTheme(): ThemeMode {
  const v = localStorage.getItem(KEY);
  return v === 'dark' || v === 'light' || v === 'system' ? v : 'system';
}

export function saveTheme(mode: ThemeMode): void {
  localStorage.setItem(KEY, mode);
}

export function resolveTheme(mode: ThemeMode): 'dark' | 'light' {
  if (mode === 'system') {
    return window.matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark';
  }
  return mode;
}

export function applyTheme(mode: ThemeMode): void {
  document.documentElement.setAttribute('data-theme', resolveTheme(mode));
}
