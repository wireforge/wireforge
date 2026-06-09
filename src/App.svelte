<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import RequestEditor from './lib/RequestEditor.svelte';
  import ResponseViewer from './lib/ResponseViewer.svelte';
  import CommandPalette from './lib/CommandPalette.svelte';
  import { loadTheme, saveTheme, applyTheme, type ThemeMode } from './lib/theme';
  import type { UnifiedRequest, UnifiedResponse, WfError } from './lib/types';

  // --- Tabs (one open request each) ---
  interface Tab {
    id: number;
    request: UnifiedRequest;
    pristine: string;
    response: UnifiedResponse | null;
    error: WfError | null;
    sending: boolean;
  }

  let nextId = 1;

  function newRequest(): UnifiedRequest {
    return {
      method: 'GET',
      url: 'https://jsonplaceholder.typicode.com/todos/1',
      params: [],
      headers: [{ enabled: true, key: 'Accept', value: 'application/json' }],
      auth: { type: 'none' },
      body: { mode: 'none' },
    };
  }

  function makeTab(): Tab {
    const request = newRequest();
    return { id: nextId++, request, pristine: JSON.stringify(request), response: null, error: null, sending: false };
  }

  let tabs = $state<Tab[]>([makeTab()]);
  let activeIndex = $state(0);
  const active = $derived(tabs[activeIndex]);

  function addTab() {
    tabs = [...tabs, makeTab()];
    activeIndex = tabs.length - 1;
  }

  function closeTab(i: number) {
    tabs = tabs.filter((_, idx) => idx !== i);
    if (tabs.length === 0) tabs = [makeTab()];
    if (activeIndex >= tabs.length) activeIndex = tabs.length - 1;
  }

  function tabLabel(t: Tab): string {
    try {
      const u = new URL(t.request.url);
      return u.pathname.split('/').filter(Boolean).pop() || u.host;
    } catch {
      return 'New request';
    }
  }

  function isDirty(t: Tab): boolean {
    return JSON.stringify(t.request) !== t.pristine;
  }

  async function send() {
    const t = tabs[activeIndex];
    if (!t || t.sending) return;
    t.sending = true;
    t.error = null;
    try {
      t.response = await invoke<UnifiedResponse>('send_request', { request: t.request });
    } catch (e) {
      t.error = e as WfError;
      t.response = null;
    } finally {
      t.sending = false;
    }
  }

  function focusUrl() {
    window.dispatchEvent(new Event('wf:focus-url'));
  }

  // --- Theme ---
  let theme = $state<ThemeMode>(loadTheme());
  $effect(() => {
    applyTheme(theme);
    saveTheme(theme);
    if (theme === 'system') {
      const mq = window.matchMedia('(prefers-color-scheme: light)');
      const onChange = () => applyTheme('system');
      mq.addEventListener('change', onChange);
      return () => mq.removeEventListener('change', onChange);
    }
  });

  // --- Layout ---
  type Orientation = 'row' | 'column';
  const LKEY = 'wf.layout';

  function loadLayout() {
    try {
      const v = JSON.parse(localStorage.getItem(LKEY) ?? '{}');
      return {
        sidebarCollapsed: !!v.sidebarCollapsed,
        sidebarWidth: typeof v.sidebarWidth === 'number' ? v.sidebarWidth : 240,
        splitRatio: typeof v.splitRatio === 'number' ? v.splitRatio : 0.5,
        orientation: (v.orientation === 'column' ? 'column' : 'row') as Orientation,
      };
    } catch {
      return { sidebarCollapsed: false, sidebarWidth: 240, splitRatio: 0.5, orientation: 'row' as Orientation };
    }
  }

  const init = loadLayout();
  let sidebarCollapsed = $state(init.sidebarCollapsed);
  let sidebarWidth = $state(init.sidebarWidth);
  let splitRatio = $state(init.splitRatio);
  let orientation = $state<Orientation>(init.orientation);

  $effect(() => {
    localStorage.setItem(
      LKEY,
      JSON.stringify({ sidebarCollapsed, sidebarWidth, splitRatio, orientation }),
    );
  });

  let mainEl = $state<HTMLElement>();
  const clamp = (n: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, n));

  function startSidebarResize(e: PointerEvent) {
    e.preventDefault();
    const startX = e.clientX;
    const startW = sidebarWidth;
    const move = (ev: PointerEvent) => {
      sidebarWidth = clamp(startW + ev.clientX - startX, 180, 480);
    };
    const up = () => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
  }

  function startSplitResize(e: PointerEvent) {
    e.preventDefault();
    const move = (ev: PointerEvent) => {
      if (!mainEl) return;
      const r = mainEl.getBoundingClientRect();
      splitRatio =
        orientation === 'row'
          ? clamp((ev.clientX - r.left) / r.width, 0.15, 0.85)
          : clamp((ev.clientY - r.top) / r.height, 0.15, 0.85);
    };
    const up = () => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
  }

  // --- Command palette + central shortcut dispatch ---
  let paletteOpen = $state(false);

  const commands = $derived([
    { id: 'send', title: 'Send request', combo: 'Ctrl/Cmd+Enter', run: send },
    { id: 'newtab', title: 'New tab', combo: 'Ctrl/Cmd+T', run: addTab },
    { id: 'closetab', title: 'Close tab', combo: 'Ctrl/Cmd+W', run: () => closeTab(activeIndex) },
    { id: 'layout', title: 'Toggle request/response layout', combo: 'Ctrl/Cmd+\\', run: () => (orientation = orientation === 'row' ? 'column' : 'row') },
    { id: 'sidebar', title: 'Toggle sidebar', combo: 'Ctrl/Cmd+B', run: () => (sidebarCollapsed = !sidebarCollapsed) },
    { id: 'focusurl', title: 'Focus URL', combo: 'Ctrl/Cmd+L', run: focusUrl },
    { id: 'theme-dark', title: 'Theme: Dark', run: () => (theme = 'dark') },
    { id: 'theme-light', title: 'Theme: Light', run: () => (theme = 'light') },
    { id: 'theme-system', title: 'Theme: System', run: () => (theme = 'system') },
  ]);

  function onKeydown(e: KeyboardEvent) {
    const mod = e.ctrlKey || e.metaKey;
    if (mod && e.key.toLowerCase() === 'k') {
      e.preventDefault();
      paletteOpen = !paletteOpen;
      return;
    }
    if (paletteOpen || !mod) return;
    const k = e.key.toLowerCase();
    if (k === 'enter') {
      e.preventDefault();
      send();
    } else if (k === 't') {
      e.preventDefault();
      addTab();
    } else if (k === 'w') {
      e.preventDefault();
      closeTab(activeIndex);
    } else if (k === '\\') {
      e.preventDefault();
      orientation = orientation === 'row' ? 'column' : 'row';
    } else if (k === 'b') {
      e.preventDefault();
      sidebarCollapsed = !sidebarCollapsed;
    } else if (k === 'l') {
      e.preventDefault();
      focusUrl();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<main class="shell">
  <header class="topbar">
    <button class="icon" onclick={() => (sidebarCollapsed = !sidebarCollapsed)} title="Toggle sidebar">☰</button>
    <span class="wordmark">wireforge</span>
    <span class="spacer"></span>
    <button class="icon" onclick={() => (paletteOpen = true)} title="Command palette (Ctrl/Cmd+K)">⌘K</button>
    <button
      class="icon"
      onclick={() => (orientation = orientation === 'row' ? 'column' : 'row')}
      title="Toggle request/response layout (Ctrl/Cmd+\)"
    >
      {orientation === 'row' ? 'Side-by-side' : 'Stacked'}
    </button>
    <select class="theme" bind:value={theme} aria-label="Theme">
      <option value="dark">Dark</option>
      <option value="light">Light</option>
      <option value="system">System</option>
    </select>
  </header>

  <div class="tabbar">
    {#each tabs as t, i (t.id)}
      <div class="tab" class:active={i === activeIndex}>
        <button class="tab-select" onclick={() => (activeIndex = i)}>
          <span class="m mono">{t.request.method}</span>
          <span class="label">{tabLabel(t)}</span>
          {#if isDirty(t)}<span class="dot" title="Unsaved">•</span>{/if}
        </button>
        <button class="tab-close" onclick={() => closeTab(i)} aria-label="Close tab">✕</button>
      </div>
    {/each}
    <button class="newtab" onclick={addTab} aria-label="New tab">+</button>
  </div>

  <div class="body">
    {#if !sidebarCollapsed}
      <aside class="sidebar" style="width: {sidebarWidth}px">Collection</aside>
      <div class="resizer vertical" role="separator" aria-orientation="vertical" onpointerdown={startSidebarResize}></div>
    {/if}

    <div class="main" bind:this={mainEl} style="flex-direction: {orientation}">
      <section class="pane editor" style="flex: {splitRatio}">
        <RequestEditor bind:request={tabs[activeIndex].request} sending={active.sending} onsend={send} />
      </section>
      <div
        class="resizer {orientation === 'row' ? 'vertical' : 'horizontal'}"
        role="separator"
        aria-orientation={orientation === 'row' ? 'vertical' : 'horizontal'}
        onpointerdown={startSplitResize}
      ></div>
      <section class="pane response" style="flex: {1 - splitRatio}">
        <ResponseViewer response={active.response} error={active.error} sending={active.sending} />
      </section>
    </div>
  </div>
</main>

<CommandPalette bind:open={paletteOpen} {commands} />

<style>
  .shell {
    display: flex;
    flex-direction: column;
    height: 100%;
  }
  .topbar {
    display: flex;
    align-items: center;
    gap: 10px;
    height: 44px;
    padding: 0 12px;
    border-bottom: 1px solid var(--border);
    background: var(--surface);
  }
  .wordmark {
    font-weight: 600;
  }
  .spacer {
    flex: 1;
  }
  .topbar .icon {
    background: transparent;
    border: 1px solid var(--border);
    color: var(--text-muted);
    border-radius: 6px;
    padding: 4px 10px;
    cursor: pointer;
    font-size: 12px;
  }
  .topbar .theme {
    background: var(--surface-code);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 4px 6px;
    font-size: 12px;
  }
  .tabbar {
    display: flex;
    align-items: stretch;
    gap: 2px;
    height: 34px;
    padding: 0 8px;
    border-bottom: 1px solid var(--border);
    background: var(--surface);
    overflow-x: auto;
  }
  .tab {
    display: flex;
    align-items: center;
    border: 1px solid transparent;
    border-bottom: none;
    border-radius: 6px 6px 0 0;
  }
  .tab.active {
    background: var(--bg);
    border-color: var(--border);
  }
  .tab-select {
    display: flex;
    align-items: center;
    gap: 6px;
    background: transparent;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 0 4px 0 10px;
    font-size: 12px;
    height: 100%;
  }
  .tab.active .tab-select {
    color: var(--text);
  }
  .tab-select .m {
    color: var(--accent);
    font-size: 10px;
  }
  .tab-select .dot {
    color: var(--accent);
  }
  .tab-close {
    background: transparent;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 0 8px;
    font-size: 11px;
  }
  .newtab {
    background: transparent;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 0 10px;
    font-size: 16px;
  }
  .body {
    display: flex;
    flex: 1;
    min-height: 0;
  }
  .sidebar {
    flex: 0 0 auto;
    padding: 12px;
    overflow: auto;
    color: var(--text-muted);
  }
  .main {
    display: flex;
    flex: 1;
    min-width: 0;
    min-height: 0;
  }
  .pane {
    overflow: auto;
    padding: 12px;
    min-width: 0;
    min-height: 0;
  }
  .resizer {
    flex: 0 0 5px;
    background: transparent;
  }
  .resizer.vertical {
    cursor: col-resize;
    border-left: 1px solid var(--border);
  }
  .resizer.horizontal {
    cursor: row-resize;
    border-top: 1px solid var(--border);
  }
  .resizer:hover {
    background: var(--border);
  }
</style>
