<script lang="ts">
  import Self from './Sidebar.svelte';
  import type { TreeNode } from './types';

  let {
    nodes,
    onopen,
    activePath,
  }: { nodes: TreeNode[]; onopen: (path: string) => void; activePath?: string } = $props();

  let expanded = $state<Set<string>>(new Set());

  function toggle(id: string) {
    if (expanded.has(id)) expanded.delete(id);
    else expanded.add(id);
    expanded = new Set(expanded);
  }
</script>

<ul class="tree">
  {#each nodes as n (n.id)}
    {#if n.kind === 'folder'}
      <li>
        <button class="row" onclick={() => toggle(n.id)}>
          <span class="chev">{expanded.has(n.id) ? '▾' : '▸'}</span>
          <span class="name">{n.name}</span>
        </button>
        {#if expanded.has(n.id)}
          <div class="children">
            <Self nodes={n.children} {onopen} {activePath} />
          </div>
        {/if}
      </li>
    {:else}
      <li>
        <button class="row" class:active={n.path === activePath} onclick={() => onopen(n.path)}>
          <span class="m m-{n.method.toLowerCase()}">{n.method}</span>
          <span class="name">{n.name}</span>
        </button>
      </li>
    {/if}
  {/each}
</ul>

<style>
  .tree {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .children {
    padding-left: 12px;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 100%;
    background: transparent;
    border: none;
    color: var(--text);
    padding: 3px 6px;
    border-radius: 6px;
    cursor: pointer;
    font-size: 12px;
    text-align: left;
  }
  .row:hover {
    background: var(--surface-code);
  }
  .row.active {
    background: var(--surface-code);
    color: var(--text);
  }
  .chev {
    color: var(--text-muted);
    width: 10px;
  }
  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .m {
    font-family: var(--font-mono);
    font-size: 9px;
    font-weight: 700;
    color: var(--text-muted);
    min-width: 34px;
  }
  .m-get {
    color: var(--success);
  }
  .m-post {
    color: var(--accent);
  }
  .m-delete {
    color: var(--danger);
  }
</style>
