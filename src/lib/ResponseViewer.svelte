<script lang="ts">
  import type { UnifiedResponse, WfError } from './types';

  let {
    response,
    error,
    sending,
    onsavebody,
  }: {
    response: UnifiedResponse | null;
    error: WfError | null;
    sending: boolean;
    onsavebody?: (content: string) => void;
  } = $props();

  let tab = $state<'body' | 'headers'>('body');

  // Above this size we stop pretty-printing and only render a prefix, so a huge
  // body never freezes the UI; the full body stays available via Save.
  const LIMIT = 262144; // 256 KB

  const isLarge = $derived(!!response && response.body.length > LIMIT);

  const prettyBody = $derived.by(() => {
    if (!response) return '';
    if (response.body.length > LIMIT) return response.body.slice(0, LIMIT);
    const contentType =
      response.headers.find((h) => h.key.toLowerCase() === 'content-type')?.value ?? '';
    if (contentType.includes('json')) {
      try {
        return JSON.stringify(JSON.parse(response.body), null, 2);
      } catch {
        return response.body;
      }
    }
    return response.body;
  });

  function statusClass(status: number): string {
    if (status >= 200 && status < 300) return 'ok';
    if (status >= 400) return 'err';
    return 'warn';
  }
</script>

{#if sending}
  <div class="state">Sending…</div>
{:else if error}
  <div class="error">
    <div class="err-code mono">{error.code}</div>
    <div class="err-msg">{error.message}</div>
  </div>
{:else if response}
  <div class="meta mono">
    <span class="status {statusClass(response.status)}">{response.status} {response.statusText}</span>
    <span>{response.durationMs} ms</span>
    <span>{response.size} B</span>
    {#if response.httpVersion}<span>{response.httpVersion}</span>{/if}
    {#if response.remoteIp}<span>{response.remoteIp}</span>{/if}
  </div>

  <div class="tabs">
    <button class:active={tab === 'body'} onclick={() => (tab = 'body')}>Body</button>
    <button class:active={tab === 'headers'} onclick={() => (tab = 'headers')}>
      Headers ({response.headers.length})
    </button>
  </div>

  {#if tab === 'body'}
    {#if isLarge}
      <div class="large-note">
        <span>
          Large response — showing the first {Math.round(LIMIT / 1024)} KB of {response.body.length.toLocaleString()} characters.
        </span>
        {#if onsavebody}
          <button class="save" onclick={() => onsavebody?.(response.body)}>Save full body…</button>
        {/if}
      </div>
    {/if}
    <pre class="body mono">{prettyBody}</pre>
  {:else}
    <table class="headers mono">
      <tbody>
        {#each response.headers as h, i (i)}
          <tr><td>{h.key}</td><td>{h.value}</td></tr>
        {/each}
      </tbody>
    </table>
  {/if}
{:else}
  <div class="state">No response yet. Send a request.</div>
{/if}

<style>
  .state {
    color: var(--text-muted);
    padding: 8px 0;
  }
  .meta {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    align-items: center;
    padding-bottom: 8px;
    border-bottom: 1px solid var(--border);
    color: var(--text-muted);
    font-size: 12px;
  }
  .status {
    font-weight: 700;
  }
  .status.ok {
    color: var(--success);
  }
  .status.warn {
    color: var(--warning);
  }
  .status.err {
    color: var(--danger);
  }
  .tabs {
    display: flex;
    gap: 4px;
    margin: 8px 0;
  }
  .tabs button {
    background: transparent;
    color: var(--text-muted);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 4px 10px;
    cursor: pointer;
  }
  .tabs button.active {
    color: var(--text);
    border-color: var(--accent);
  }
  .large-note {
    display: flex;
    align-items: center;
    gap: 10px;
    justify-content: space-between;
    background: var(--surface-code);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 6px 10px;
    margin-bottom: 8px;
    font-size: 12px;
    color: var(--text-muted);
  }
  .large-note .save {
    flex: 0 0 auto;
    background: transparent;
    color: var(--accent);
    border: 1px solid var(--border);
    border-radius: 6px;
    padding: 3px 10px;
    cursor: pointer;
    font-size: 12px;
  }
  .body {
    margin: 0;
    white-space: pre-wrap;
    word-break: break-word;
    font-size: 12px;
    color: var(--text);
  }
  .headers {
    border-collapse: collapse;
    font-size: 12px;
    width: 100%;
  }
  .headers td {
    border-bottom: 1px solid var(--border);
    padding: 3px 8px 3px 0;
    vertical-align: top;
    color: var(--text);
  }
  .headers td:first-child {
    color: var(--text-muted);
    white-space: nowrap;
  }
  .error {
    border: 1px solid var(--danger);
    border-radius: 8px;
    padding: 12px;
  }
  .err-code {
    color: var(--danger);
    font-weight: 700;
    margin-bottom: 4px;
  }
  .mono {
    font-family: var(--font-mono);
  }
</style>
