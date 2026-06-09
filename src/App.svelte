<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import RequestEditor from './lib/RequestEditor.svelte';
  import ResponseViewer from './lib/ResponseViewer.svelte';
  import type { UnifiedRequest, UnifiedResponse, WfError } from './lib/types';

  // v0.1 prototype: a single request loop. Tabs, collection sidebar, and the
  // command palette are placeholders that grow in later phases.
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
</script>

<main class="shell">
  <header class="topbar">
    <span class="wordmark">wireforge</span>
  </header>

  <div class="panes">
    <aside class="pane sidebar">Collection</aside>
    <section class="pane editor">
      <RequestEditor bind:request {sending} onsend={send} />
    </section>
    <section class="pane response">
      <ResponseViewer {response} {error} {sending} />
    </section>
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
    gap: 12px;
    height: 44px;
    padding: 0 12px;
    border-bottom: 1px solid var(--border);
    background: var(--surface);
  }
  .wordmark {
    font-weight: 600;
  }
  .panes {
    display: grid;
    grid-template-columns: 220px 1fr 1fr;
    flex: 1;
    min-height: 0;
  }
  .pane {
    padding: 12px;
    overflow: auto;
  }
  .sidebar {
    border-right: 1px solid var(--border);
    color: var(--text-muted);
  }
  .editor {
    border-right: 1px solid var(--border);
  }
</style>
