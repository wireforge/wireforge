<script lang="ts">
  interface Command {
    id: string;
    title: string;
    combo?: string;
    run: () => void;
  }

  let { open = $bindable(), commands }: { open: boolean; commands: Command[] } = $props();

  let query = $state('');
  let index = $state(0);
  let inputEl = $state<HTMLInputElement>();

  const filtered = $derived(
    commands.filter((c) => c.title.toLowerCase().includes(query.toLowerCase().trim())),
  );

  $effect(() => {
    if (open) {
      query = '';
      index = 0;
      const el = inputEl;
      queueMicrotask(() => el?.focus());
    }
  });

  function run(c: Command) {
    open = false;
    c.run();
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      open = false;
    } else if (e.key === 'ArrowDown') {
      e.preventDefault();
      index = Math.min(index + 1, filtered.length - 1);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      index = Math.max(index - 1, 0);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      const c = filtered[index];
      if (c) run(c);
    }
  }
</script>

{#if open}
  <div class="cp">
    <button class="backdrop" aria-label="Close command palette" onclick={() => (open = false)}></button>
    <div class="palette" role="dialog" aria-modal="true" aria-label="Command palette">
      <input
        bind:this={inputEl}
        bind:value={query}
        oninput={() => (index = 0)}
        onkeydown={onkeydown}
        placeholder="Type a command…"
        spellcheck="false"
        autocomplete="off"
      />
      <ul>
        {#each filtered as c, i (c.id)}
          <li>
            <button
              class="row"
              class:active={i === index}
              onclick={() => run(c)}
              onmouseenter={() => (index = i)}
            >
              <span class="title">{c.title}</span>
              {#if c.combo}<span class="combo mono">{c.combo}</span>{/if}
            </button>
          </li>
        {/each}
        {#if filtered.length === 0}
          <li class="empty">No matching commands</li>
        {/if}
      </ul>
    </div>
  </div>
{/if}

<style>
  .cp {
    position: fixed;
    inset: 0;
    z-index: 100;
    display: flex;
    justify-content: center;
    align-items: flex-start;
  }
  .backdrop {
    position: absolute;
    inset: 0;
    background: rgba(0, 0, 0, 0.4);
    border: none;
    padding: 0;
    cursor: default;
  }
  .palette {
    position: relative;
    margin-top: 12vh;
    width: 540px;
    max-width: 90vw;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 10px;
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.4);
    overflow: hidden;
  }
  .palette input {
    width: 100%;
    box-sizing: border-box;
    background: var(--surface-code);
    color: var(--text);
    border: none;
    border-bottom: 1px solid var(--border);
    padding: 12px 14px;
    font-size: 14px;
  }
  .palette input:focus {
    outline: none;
  }
  ul {
    list-style: none;
    margin: 0;
    padding: 4px;
    max-height: 50vh;
    overflow: auto;
  }
  .row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    width: 100%;
    background: transparent;
    border: none;
    color: var(--text);
    padding: 8px 10px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 13px;
    text-align: left;
  }
  .row.active {
    background: var(--surface-code);
  }
  .combo {
    color: var(--text-muted);
    font-size: 11px;
  }
  .empty {
    color: var(--text-muted);
    padding: 8px 10px;
    font-size: 13px;
  }
  .mono {
    font-family: var(--font-mono);
  }
</style>
