<script lang="ts">
  import type { UnifiedRequest } from './types';

  let {
    request = $bindable(),
    sending,
    onsend,
  }: { request: UnifiedRequest; sending: boolean; onsend: () => void } = $props();

  const methods = ['GET', 'POST', 'PUT', 'PATCH', 'DELETE', 'HEAD', 'OPTIONS'];

  function addHeader() {
    request.headers = [...request.headers, { enabled: true, key: '', value: '' }];
  }

  function removeHeader(index: number) {
    request.headers = request.headers.filter((_, i) => i !== index);
  }

  function setBodyMode(mode: string) {
    if (mode === 'json') {
      request.body = { mode: 'json', text: request.body.mode === 'json' ? request.body.text : '{\n  \n}' };
    } else if (mode === 'raw') {
      request.body = {
        mode: 'raw',
        contentType: 'text/plain',
        text: request.body.mode === 'raw' ? request.body.text : '',
      };
    } else {
      request.body = { mode: 'none' };
    }
  }

  function onKeydown(event: KeyboardEvent) {
    if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') {
      event.preventDefault();
      onsend();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<div class="editor">
  <div class="bar">
    <select class="method" bind:value={request.method} aria-label="Method">
      {#each methods as m}
        <option value={m}>{m}</option>
      {/each}
    </select>
    <input
      class="url mono"
      bind:value={request.url}
      placeholder="https://api.example.com/v1/..."
      spellcheck="false"
      autocomplete="off"
      aria-label="URL"
    />
    <button class="send" onclick={onsend} disabled={sending}>
      {sending ? 'Sending…' : 'Send'}
    </button>
  </div>

  <section class="block">
    <div class="block-head">
      <span>Headers</span>
      <button class="ghost" onclick={addHeader}>+ Add</button>
    </div>
    {#each request.headers as header, i (i)}
      <div class="kv">
        <input type="checkbox" bind:checked={header.enabled} aria-label="Enabled" />
        <input class="mono" bind:value={header.key} placeholder="Header" spellcheck="false" />
        <input class="mono" bind:value={header.value} placeholder="Value" spellcheck="false" />
        <button class="ghost" onclick={() => removeHeader(i)} aria-label="Remove header">✕</button>
      </div>
    {/each}
  </section>

  <section class="block">
    <div class="block-head">
      <span>Body</span>
      <select value={request.body.mode} onchange={(e) => setBodyMode(e.currentTarget.value)} aria-label="Body mode">
        <option value="none">None</option>
        <option value="json">JSON</option>
        <option value="raw">Raw</option>
      </select>
    </div>
    {#if request.body.mode === 'json' || request.body.mode === 'raw'}
      <textarea class="body mono" bind:value={request.body.text} spellcheck="false"></textarea>
    {/if}
  </section>
</div>

<style>
  .editor {
    display: flex;
    flex-direction: column;
    gap: 16px;
    height: 100%;
  }
  .bar {
    display: flex;
    gap: 8px;
  }
  .method {
    flex: 0 0 auto;
  }
  .url {
    flex: 1;
  }
  select,
  input,
  textarea {
    background: var(--surface-code);
    color: var(--text);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 6px 8px;
    font-size: 12px;
  }
  input[type='checkbox'] {
    padding: 0;
  }
  .send {
    background: var(--accent);
    color: #0f1318;
    border: none;
    border-radius: 6px;
    padding: 6px 16px;
    font-weight: 600;
    cursor: pointer;
  }
  .send:disabled {
    opacity: 0.6;
    cursor: default;
  }
  .block-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 6px;
    color: var(--text-muted);
  }
  .kv {
    display: grid;
    grid-template-columns: auto 1fr 1fr auto;
    gap: 6px;
    margin-bottom: 4px;
  }
  .ghost {
    background: transparent;
    color: var(--text-muted);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 2px 8px;
    cursor: pointer;
  }
  .body {
    width: 100%;
    min-height: 160px;
    resize: vertical;
  }
  .mono {
    font-family: var(--font-mono);
  }
</style>
