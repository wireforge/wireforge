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

// --- Custom themes (THEME_FORMAT) ---

// The editable tokens the app actually renders, mapped to their CSS variables.
export const TOKEN_VARS: Record<string, string> = {
  bg: '--bg',
  surface: '--surface',
  surfaceCode: '--surface-code',
  border: '--border',
  text: '--text',
  textMuted: '--text-muted',
  accent: '--accent',
  accentBright: '--accent-bright',
  success: '--success',
  warning: '--warning',
  danger: '--danger',
};

export interface Theme {
  format: string;
  version: number;
  id: string;
  name: string;
  base: 'dark' | 'light';
  tokens: Record<string, string>;
  [extra: string]: unknown;
}

const CUSTOM_KEY = 'wf.customThemes';
const ACTIVE_KEY = 'wf.activeTheme';

export function loadCustomThemes(): Theme[] {
  try {
    const v = JSON.parse(localStorage.getItem(CUSTOM_KEY) ?? '[]');
    return Array.isArray(v) ? v : [];
  } catch {
    return [];
  }
}

export function saveCustomThemes(themes: Theme[]): void {
  localStorage.setItem(CUSTOM_KEY, JSON.stringify(themes));
}

export function loadActiveThemeId(): string {
  return localStorage.getItem(ACTIVE_KEY) ?? '';
}

export function saveActiveThemeId(id: string): void {
  localStorage.setItem(ACTIVE_KEY, id);
}

/// Read the currently-rendered token values (to seed a duplicate).
export function currentTokens(): Record<string, string> {
  const cs = getComputedStyle(document.documentElement);
  const out: Record<string, string> = {};
  for (const [k, v] of Object.entries(TOKEN_VARS)) {
    out[k] = (cs.getPropertyValue(v).trim() || '#000000').toLowerCase();
  }
  return out;
}

export function applyCustomTheme(theme: Theme): void {
  document.documentElement.setAttribute('data-theme', theme.base);
  const s = document.documentElement.style;
  for (const [k, v] of Object.entries(TOKEN_VARS)) {
    const val = theme.tokens[k];
    if (val) s.setProperty(v, val);
  }
}

export function clearCustomTheme(): void {
  const s = document.documentElement.style;
  for (const v of Object.values(TOKEN_VARS)) s.removeProperty(v);
}

export function newThemeId(): string {
  return `theme_${Math.random().toString(36).slice(2, 10)}`;
}

/// Validate and normalize an imported theme. Throws a message on bad input.
export function parseTheme(json: string): Theme {
  let v: unknown;
  try {
    v = JSON.parse(json);
  } catch {
    throw new Error('Not valid JSON.');
  }
  const t = v as Partial<Theme>;
  if (t.format !== 'wireforge.theme') throw new Error('Not a wireforge.theme file.');
  if (t.base !== 'dark' && t.base !== 'light') throw new Error('Theme "base" must be dark or light.');
  if (!t.tokens || typeof t.tokens !== 'object') throw new Error('Theme is missing tokens.');
  return {
    ...(v as Theme),
    version: typeof t.version === 'number' ? t.version : 1,
    id: typeof t.id === 'string' && t.id ? t.id : newThemeId(),
    name: typeof t.name === 'string' && t.name ? t.name : 'Imported theme',
  };
}
