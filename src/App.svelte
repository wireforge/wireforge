<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import RequestEditor from './lib/RequestEditor.svelte';
  import ResponseViewer from './lib/ResponseViewer.svelte';
  import { loadTheme, saveTheme, applyTheme, type ThemeMode } from './lib/theme';
  import type { UnifiedRequest, UnifiedResponse, WfError } from './lib/types';

  // --- Request state (single request for now; tabs land in the next chunk) ---
  let request = $state<UnifiedRequest>({
    method: 'GET',
    url: 'https://jsonplaceholder.typicode.com/todos/1',
    params: [],
    headers: [{ enabled: true, key: 'Accept', value: 'application/json' }],
    auth: { type: 'none' },
    body: { mode: 'none' },
  });
  let response = $state<UnifiedResponse | null>(null);
  let error = $state<WfError | null>(null);
  let sending = $state(false);

  async function send() {
    if (sending) return;
    sending = true;
    error = null;
    try {
      response = await invoke<UnifiedResponse>('send_request', { request });
    } catch (e) {
      error = e as WfError;
      response = null;
    } finally {
      sending = false;
    }
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

  // --- Layout (persisted per browser; "per workspace" arrives with workspaces) ---
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

  function onKeydown(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key === '\\') {
      e.preventDefault();
      orientation = orientation === 'row' ? 'column' : 'row';
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<main class="shell">
  <header class="topbar">
    <button class="icon" onclick={() => (sidebarCollapsed = !sidebarCollapsed)} title="Toggle sidebar">☰</button>
    <span class="wordmark">wireforge</span>
    <span class="spacer"></span>
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

  <div class="body">
    {#if !sidebarCollapsed}
      <aside class="sidebar" style="width: {sidebarWidth}px">Collection</aside>
      <div
        class="resizer vertical"
        role="separator"
        aria-orientation="vertical"
        onpointerdown={startSidebarResize}
      ></div>
    {/if}

    <div class="main" bind:this={mainEl} style="flex-direction: {orientation}">
      <section class="pane editor" style="flex: {splitRatio}">
        <RequestEditor bind:request {sending} onsend={send} />
      </section>
      <div
        class="resizer {orientation === 'row' ? 'vertical' : 'horizontal'}"
        role="separator"
        aria-orientation={orientation === 'row' ? 'vertical' : 'horizontal'}
        onpointerdown={startSplitResize}
      ></div>
      <section class="pane response" style="flex: {1 - splitRatio}">
        <ResponseViewer {response} {error} {sending} />
      </section>
    </div>
  </div>
</main>

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
