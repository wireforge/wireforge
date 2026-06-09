<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';

  // v0.1 prototype shell. The three panes are placeholders; real components
  // (CollectionSidebar, RequestEditor, ResponseViewer) land in later phases.
  let info = $state('connecting…');

  async function loadInfo() {
    try {
      info = await invoke<string>('app_info');
    } catch (e) {
      info = `backend error: ${e}`;
    }
  }

  loadInfo();
</script>

<main class="shell">
  <header class="topbar">
    <span class="wordmark">wireforge</span>
    <span class="info">{info}</span>
  </header>

  <div class="panes">
    <aside class="pane sidebar">Collection</aside>
    <section class="pane editor">Request</section>
    <section class="pane response">Response</section>
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

  .info {
    color: var(--text-muted);
    font-family: var(--font-mono);
    font-size: 11px;
  }

  .panes {
    display: grid;
    grid-template-columns: 280px 1fr 1fr;
    flex: 1;
    min-height: 0;
  }

  .pane {
    padding: 12px;
    overflow: auto;
    color: var(--text-muted);
  }

  .sidebar {
    border-right: 1px solid var(--border);
  }

  .editor {
    border-right: 1px solid var(--border);
  }
</style>
