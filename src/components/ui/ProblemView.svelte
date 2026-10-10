<script lang="ts">
  import { Copy, FileText, FolderOpen, RotateCcw, ExternalLink, Settings as SettingsIcon } from '@lucide/svelte';
  import { api } from '../../lib/api';
  import { app, type PageId } from '../../lib/store.svelte';
  import type { Problem } from '../../lib/types';
  import Button from './Button.svelte';

  interface Props {
    problem: Problem;
    onretry?: () => void;
    compact?: boolean;
  }
  let { problem, onretry, compact = false }: Props = $props();
  let copied = $state(false);

  async function copyDetails() {
    const text = [
      `${problem.title} [${problem.code}]`,
      `What happened: ${problem.what}`,
      problem.why && `Possible reason: ${problem.why}`,
      problem.fix && `Suggested fix: ${problem.fix}`,
      problem.details && `Details: ${problem.details}`,
    ]
      .filter(Boolean)
      .join('\n');
    await navigator.clipboard.writeText(text);
    copied = true;
    setTimeout(() => (copied = false), 1600);
  }

  function openSettings(target: string) {
    const pages: Record<string, [PageId, string | null]> = {
      flags: ['flags', null],
      roblox: ['settings', 'roblox'],
      recovery: ['settings', 'recovery'],
    };
    const [page, section] = pages[target] ?? ['settings', target];
    app.problem = null;
    app.go(page, section);
  }
</script>

<div class="problem" class:compact role="alert">
  <div class="title">{problem.title}</div>
  <dl>
    <dt>What happened</dt>
    <dd>{problem.what}</dd>
    {#if problem.why}<dt>Possible reason</dt>
      <dd>{problem.why}</dd>{/if}
    {#if problem.fix}<dt>What to do</dt>
      <dd>{problem.fix}</dd>{/if}
  </dl>
  {#if problem.details}
    <details>
      <summary>Technical details</summary>
      <pre>{problem.details}</pre>
    </details>
  {/if}
  <div class="actions">
    {#each problem.actions as a, i (i)}
      {#if a.kind === 'retry' && onretry}
        <Button size="sm" variant="primary" icon={RotateCcw} onclick={onretry}>Try again</Button>
      {:else if a.kind === 'openUrl'}
        <Button size="sm" icon={ExternalLink} onclick={() => api.openUrl(a.value)}
          >Open {new URL(a.value).hostname}</Button
        >
      {:else if a.kind === 'openSettings'}
        <Button size="sm" icon={SettingsIcon} onclick={() => openSettings(a.value)}>Open settings</Button>
      {:else if a.kind === 'openConfigFolder'}
        <Button size="sm" icon={FolderOpen} onclick={() => api.openFolder('config')}>Open config folder</Button>
      {/if}
    {/each}
    <Button size="sm" variant="ghost" icon={Copy} onclick={copyDetails}>{copied ? 'Copied' : 'Copy details'}</Button>
    {#if problem.actions.some((a) => a.kind === 'openLogs')}
      <Button size="sm" variant="ghost" icon={FileText} onclick={() => api.openFolder('logs')}>Open logs</Button>
    {/if}
  </div>
</div>

<style>
  .problem {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }
  .title {
    font-weight: 650;
    font-size: var(--text-lg);
    color: var(--text);
  }
  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 6px var(--space-4);
    margin: 0;
  }
  dt {
    color: var(--text-3);
    font-size: var(--text-sm);
    padding-top: 1px;
  }
  dd {
    margin: 0;
    color: var(--text-2);
  }
  .compact dl {
    grid-template-columns: 1fr;
    gap: 2px;
  }
  .compact dd {
    margin-bottom: 6px;
  }
  details {
    font-size: var(--text-sm);
    color: var(--text-3);
  }
  summary {
    cursor: pointer;
  }
  pre {
    margin: 8px 0 0;
    padding: 10px 12px;
    border-radius: var(--radius-sm);
    background: var(--surface);
    border: 1px solid var(--border);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    white-space: pre-wrap;
    word-break: break-word;
  }
  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: var(--space-2);
  }
</style>
