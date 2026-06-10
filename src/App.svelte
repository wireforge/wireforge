<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import { open, confirm } from '@tauri-apps/plugin-dialog';
  import RequestEditor from './lib/RequestEditor.svelte';
  import ResponseViewer from './lib/ResponseViewer.svelte';
  import CommandPalette from './lib/CommandPalette.svelte';
  import Sidebar from './lib/Sidebar.svelte';
  import ImportReview from './lib/ImportReview.svelte';
  import EnvManager from './lib/EnvManager.svelte';
  import { loadTheme, saveTheme, applyTheme, type ThemeMode } from './lib/theme';
  import type {
    UnifiedRequest,
    UnifiedResponse,
    RequestFile,
    TreeNode,
    WfError,
    ImportPreview,
    ImportResult,
    EnvSummary,
    ResolveOutcome,
  } from './lib/types';

  // --- Tabs (file-backed when opened from the collection) ---
  interface Tab {
    id: number;
    request: UnifiedRequest;
    pristine: string;
    response: UnifiedResponse | null;
    error: WfError | null;
    sending: boolean;
    file?: { path: string; id: string; name: string };
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
    if (t.file) return t.file.name;
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
      t.response = await invoke<UnifiedResponse>('send_request', {
        request: t.request,
        root: workspaceRoot ?? undefined,
        environment: activeEnv ?? undefined,
      });
    } catch (e) {
      const err = e as WfError;
      t.error = err;
      t.response = null;
      // Pre-send validation failures route the user to where they can fix it.
      if (err?.code === 'WF_SECRET_MISSING') openEnvManager(true);
    } finally {
      t.sending = false;
    }
  }

  function focusUrl() {
    window.dispatchEvent(new Event('wf:focus-url'));
  }

  // --- Workspace + collection tree ---
  let workspaceRoot = $state<string | null>(localStorage.getItem('wf.workspace'));
  let tree = $state<TreeNode[]>([]);

  $effect(() => {
    const root = workspaceRoot;
    if (!root) {
      tree = [];
      return;
    }
    invoke<TreeNode[]>('open_workspace', { root })
      .then((t) => (tree = t))
      .catch(() => (tree = []));
  });

  async function openWorkspace() {
    try {
      const dir = await open({ directory: true, title: 'Open workspace folder' });
      if (typeof dir === 'string') {
        workspaceRoot = dir;
        localStorage.setItem('wf.workspace', dir);
      }
    } catch {
      // dialog unavailable (e.g. plain browser preview)
    }
  }

  async function openRequest(path: string) {
    if (!workspaceRoot) return;
    const existing = tabs.findIndex((t) => t.file?.path === path);
    if (existing >= 0) {
      activeIndex = existing;
      return;
    }
    try {
      const rf = await invoke<RequestFile>('load_request_file', { root: workspaceRoot, path });
      const request: UnifiedRequest = {
        method: rf.method,
        url: rf.url,
        params: rf.params ?? [],
        headers: rf.headers ?? [],
        auth: rf.auth ?? { type: 'none' },
        body: rf.body ?? { mode: 'none' },
      };
      tabs = [
        ...tabs,
        {
          id: nextId++,
          request,
          pristine: JSON.stringify(request),
          response: null,
          error: null,
          sending: false,
          file: { path, id: rf.id, name: rf.name },
        },
      ];
      activeIndex = tabs.length - 1;
    } catch {
      // ignore load failure for now
    }
  }

  async function save() {
    const t = tabs[activeIndex];
    if (!t?.file || !workspaceRoot) return;
    const rf: RequestFile = {
      format: 'wireforge.request',
      version: 1,
      id: t.file.id,
      name: t.file.name,
      method: t.request.method,
      url: t.request.url,
      params: t.request.params,
      headers: t.request.headers,
      auth: t.request.auth,
      body: t.request.body,
    };
    try {
      await invoke('save_request_file', { root: workspaceRoot, path: t.file.path, request: rf });
      t.pristine = JSON.stringify(t.request);
    } catch {
      // surface save errors in a later pass
    }
  }

  // --- Collection CRUD ---
  let query = $state('');

  function filterTree(input: TreeNode[], q: string): TreeNode[] {
    const ql = q.trim().toLowerCase();
    if (!ql) return input;
    const out: TreeNode[] = [];
    for (const n of input) {
      if (n.kind === 'request') {
        if (n.name.toLowerCase().includes(ql) || n.method.toLowerCase().includes(ql)) out.push(n);
      } else {
        const nameHit = n.name.toLowerCase().includes(ql);
        const kids = filterTree(n.children, q);
        if (nameHit || kids.length) out.push({ ...n, children: nameHit ? n.children : kids });
      }
    }
    return out;
  }

  const filteredTree = $derived(filterTree(tree, query));

  async function refreshTree() {
    if (!workspaceRoot) return;
    try {
      tree = await invoke<TreeNode[]>('open_workspace', { root: workspaceRoot });
    } catch {
      tree = [];
    }
  }

  async function createRequest(folder = '') {
    if (!workspaceRoot) return;
    try {
      const path = await invoke<string>('create_request', { root: workspaceRoot, folder, name: 'New request' });
      await refreshTree();
      await openRequest(path);
    } catch {
      // ignore
    }
  }

  async function createFolder(parent = '') {
    if (!workspaceRoot) return;
    try {
      await invoke('create_folder', { root: workspaceRoot, parent, name: 'New folder' });
      await refreshTree();
    } catch {
      // ignore
    }
  }

  async function renameNode(path: string, name: string) {
    if (!workspaceRoot) return;
    try {
      await invoke('rename_node', { root: workspaceRoot, path, name });
      const t = tabs.find((x) => x.file?.path === path);
      if (t?.file) t.file.name = name;
      await refreshTree();
    } catch {
      // ignore
    }
  }

  async function deleteNode(path: string) {
    if (!workspaceRoot) return;
    let ok = true;
    try {
      ok = await confirm('Delete this item? This cannot be undone.', { title: 'wireforge', kind: 'warning' });
    } catch {
      ok = true;
    }
    if (!ok) return;
    try {
      await invoke('delete_node', { root: workspaceRoot, path });
      tabs = tabs.filter((t) => !(t.file && (t.file.path === path || t.file.path.startsWith(`${path}/`))));
      if (tabs.length === 0) tabs = [makeTab()];
      if (activeIndex >= tabs.length) activeIndex = tabs.length - 1;
      await refreshTree();
    } catch {
      // ignore
    }
  }

  async function duplicateNode(path: string) {
    if (!workspaceRoot) return;
    try {
      const np = await invoke<string>('duplicate_request', { root: workspaceRoot, path });
      await refreshTree();
      await openRequest(np);
    } catch {
      // ignore
    }
  }

  async function moveNode(src: string, dest: string) {
    if (!workspaceRoot || src === dest) return;
    try {
      const np = await invoke<string>('move_node', { root: workspaceRoot, path: src, dest });
      const t = tabs.find((x) => x.file?.path === src);
      if (t?.file) t.file.path = np;
      await refreshTree();
    } catch {
      // ignore (e.g. moving a folder into itself)
    }
  }

  // --- Postman import (preview → review → apply) ---
  let importOpen = $state(false);
  let importPreviewData = $state<ImportPreview | null>(null);
  let importResult = $state<ImportResult | null>(null);
  let importError = $state<string | null>(null);
  let importBusy = $state(false);
  let importFilePath: string | null = null;

  const errMsg = (e: unknown) => (e as WfError)?.message ?? String(e);

  async function importFile() {
    if (!workspaceRoot) {
      await openWorkspace();
      if (!workspaceRoot) return;
    }
    let file: string | string[] | null = null;
    try {
      file = await open({
        title: 'Import Postman collection or environment',
        filters: [{ name: 'Postman JSON', extensions: ['json'] }],
      });
    } catch {
      return; // dialog unavailable (plain browser preview)
    }
    if (typeof file !== 'string') return;

    importFilePath = file;
    importPreviewData = null;
    importResult = null;
    importError = null;
    try {
      importPreviewData = await invoke<ImportPreview>('import_preview', { path: file });
    } catch (e) {
      importError = errMsg(e);
    }
    importOpen = true;
  }

  async function confirmImport() {
    if (!workspaceRoot || !importFilePath || importBusy) return;
    importBusy = true;
    try {
      importResult = await invoke<ImportResult>('import_apply', {
        root: workspaceRoot,
        path: importFilePath,
      });
      await refreshTree();
    } catch (e) {
      importError = errMsg(e);
    } finally {
      importBusy = false;
    }
  }

  // --- Environments & secrets ---
  let environments = $state<EnvSummary[]>([]);
  let activeEnv = $state<string | null>(null);
  let envManagerOpen = $state(false);
  let envManagerFocusSecrets = $state(false);

  const envKey = (root: string) => `wf.env:${root}`;

  // Load environments and restore the active one whenever the workspace changes.
  $effect(() => {
    const root = workspaceRoot;
    if (!root) {
      environments = [];
      activeEnv = null;
      return;
    }
    const saved = localStorage.getItem(envKey(root));
    invoke<EnvSummary[]>('list_environments', { root })
      .then((list) => {
        environments = list;
        activeEnv = saved && list.some((e) => e.slug === saved) ? saved : null;
      })
      .catch(() => {
        environments = [];
        activeEnv = null;
      });
  });

  function setActiveEnv(slug: string | null) {
    activeEnv = slug;
    if (workspaceRoot) {
      if (slug) localStorage.setItem(envKey(workspaceRoot), slug);
      else localStorage.removeItem(envKey(workspaceRoot));
    }
  }

  async function refreshEnvironments() {
    if (!workspaceRoot) return;
    try {
      environments = await invoke<EnvSummary[]>('list_environments', { root: workspaceRoot });
      if (activeEnv && !environments.some((e) => e.slug === activeEnv)) setActiveEnv(null);
    } catch {
      // ignore
    }
  }

  function openEnvManager(focusSecrets = false) {
    if (!workspaceRoot) return;
    envManagerFocusSecrets = focusSecrets;
    envManagerOpen = true;
  }

  // Live preview callback for the URL field (secrets always redacted by the backend).
  async function previewInput(input: string): Promise<ResolveOutcome | null> {
    if (!workspaceRoot || !input.includes('{{')) return null;
    try {
      return await invoke<ResolveOutcome>('resolve_preview', {
        root: workspaceRoot,
        environment: activeEnv ?? undefined,
        input,
      });
    } catch {
      return null;
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
    { id: 'save', title: 'Save request', combo: 'Ctrl/Cmd+S', run: save },
    { id: 'open', title: 'Open workspace folder…', run: openWorkspace },
    { id: 'import', title: 'Import Postman file…', run: importFile },
    { id: 'envs', title: 'Manage environments & secrets…', run: () => openEnvManager(false) },
    { id: 'newreq', title: 'New request', run: () => createRequest('') },
    { id: 'newfolder', title: 'New folder', run: () => createFolder('') },
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
    } else if (k === 's') {
      e.preventDefault();
      save();
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
    {#if workspaceRoot}
      <select
        class="env-switch"
        value={activeEnv ?? ''}
        onchange={(e) => setActiveEnv(e.currentTarget.value || null)}
        title="Active environment"
        aria-label="Active environment"
      >
        <option value="">No environment</option>
        {#each environments as env (env.slug)}
          <option value={env.slug}>{env.name}</option>
        {/each}
      </select>
      <button class="icon" onclick={() => openEnvManager(false)} title="Manage environments & secrets">Env…</button>
    {/if}
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
      <aside class="sidebar" style="width: {sidebarWidth}px">
        <div class="side-head">
          <span class="side-title">Collection</span>
          <button class="ghost" onclick={openWorkspace}>{workspaceRoot ? 'Reopen' : 'Open folder'}</button>
        </div>
        {#if workspaceRoot}
          <div class="side-tools">
            <input class="search" placeholder="Search…" bind:value={query} aria-label="Search requests" />
            <button class="ghost" title="New request" aria-label="New request" onclick={() => createRequest('')}>＋ Req</button>
            <button class="ghost" title="New folder" aria-label="New folder" onclick={() => createFolder('')}>＋ Dir</button>
            <button class="ghost" title="Import Postman file" aria-label="Import Postman file" onclick={importFile}>Import</button>
          </div>
          {#if filteredTree.length}
            <Sidebar
              nodes={filteredTree}
              onopen={openRequest}
              activePath={active?.file?.path}
              {query}
              onNewRequest={(p) => createRequest(p)}
              onNewFolder={(p) => createFolder(p)}
              onRename={renameNode}
              onDelete={deleteNode}
              onDuplicate={duplicateNode}
              onMove={moveNode}
            />
          {:else}
            <p class="hint">{query ? 'No matches.' : 'Empty workspace.'}</p>
          {/if}
        {:else}
          <p class="hint">Open a folder to load requests.</p>
        {/if}
      </aside>
      <div class="resizer vertical" role="separator" aria-orientation="vertical" onpointerdown={startSidebarResize}></div>
    {/if}

    <div class="main" bind:this={mainEl} style="flex-direction: {orientation}">
      <section class="pane editor" style="flex: {splitRatio}">
        <RequestEditor
          bind:request={tabs[activeIndex].request}
          sending={active.sending}
          onsend={send}
          preview={previewInput}
        />
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

<ImportReview
  bind:open={importOpen}
  preview={importPreviewData}
  result={importResult}
  error={importError}
  busy={importBusy}
  onconfirm={confirmImport}
/>

{#if workspaceRoot}
  <EnvManager
    bind:open={envManagerOpen}
    root={workspaceRoot}
    {environments}
    focusSecrets={envManagerFocusSecrets}
    onchanged={refreshEnvironments}
  />
{/if}

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
  .topbar .theme,
  .topbar .env-switch {
    background: var(--surface-code);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 4px 6px;
    font-size: 12px;
  }
  .topbar .env-switch {
    max-width: 160px;
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
    padding: 8px;
    overflow: auto;
  }
  .side-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
  }
  .side-title {
    color: var(--text-muted);
    font-size: 12px;
  }
  .side-tools {
    display: flex;
    gap: 4px;
    margin-bottom: 8px;
  }
  .search {
    flex: 1;
    min-width: 0;
    background: var(--surface-code);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 4px 8px;
    font-size: 12px;
  }
  .ghost {
    background: transparent;
    color: var(--text-muted);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 2px 8px;
    cursor: pointer;
    font-size: 11px;
  }
  .hint {
    color: var(--text-muted);
    font-size: 12px;
    padding: 4px 6px;
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
