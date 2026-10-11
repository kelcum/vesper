<script lang="ts">
  import { ChevronDown, ExternalLink, Trash } from '@lucide/svelte';
  import { slide } from 'svelte/transition';
  import { api } from '../../lib/api';
  import { kindOf, normalizeValue, verificationBadge, type FlagRow } from '../../lib/flags';
  import type { Catalog } from '../../lib/types';
  import Badge from '../ui/Badge.svelte';
  import Button from '../ui/Button.svelte';
  import Segmented from '../ui/Segmented.svelte';

  interface Props {
    row: FlagRow;
    catalog: Catalog;
    published: string | null;
    onset: (name: string, value: string | null) => void;
  }
  let { row, catalog, published, onset }: Props = $props();

  let open = $state(false);
  const info = $derived(row.info);
  const badge = $derived(verificationBadge(row));
  const kind = $derived(kindOf(row.name));
  let draft = $state('');
  let draftError = $state<string | null>(null);
  $effect(() => {
    draft = row.value ?? '';
  });

  function commitDraft() {
    if (draft.trim() === '') {
      if (row.value !== null && info) onset(row.name, null);
      draftError = null;
      return;
    }
    const r = normalizeValue(row.name, draft);
    draftError = r.error ?? null;
    if (r.value !== undefined && r.value !== row.value) onset(row.name, r.value);
  }

  const boolValue = $derived<'default' | 'True' | 'False'>(
    row.value === 'True' ? 'True' : row.value === 'False' ? 'False' : 'default',
  );
  const defaultText = $derived.by(() => {
    if (!info) return null;
    if (published != null) return `Roblox ships ${published} today.`;
    if (info.robloxDefault) return `Roblox published ${info.robloxDefault.value} (${info.robloxDefault.observed}).`;
    return "Roblox doesn't publish a value, so its built-in default applies.";
  });
</script>

<div class="row" class:set={row.value !== null} class:open>
  <div class="main">
    <button
      class="expand"
      aria-expanded={open}
      aria-label="Details for {info?.title ?? row.name}"
      onclick={() => (open = !open)}
    >
      <ChevronDown size={15} strokeWidth={2} aria-hidden="true" />
    </button>
    <div class="copy">
      <div class="title">
        {info?.title ?? row.name}
        {#if info?.risk === 'medium'}<Badge tone="warn" title="Can noticeably change how Roblox looks or performs."
            >Medium risk</Badge
          >{/if}
        {#if info?.risk === 'high'}<Badge tone="bad">High risk</Badge>{/if}
        {#if info?.status === 'removal-announced'}<Badge tone="warn" title="Roblox said it will remove this flag."
            >Being removed</Badge
          >{/if}
      </div>
      <div class="name mono">{row.name}</div>
      {#if info}<p class="summary">{info.summary}</p>{/if}
      {#if row.ignoredReason}<p class="summary bad">{row.ignoredReason}</p>{/if}
      {#each row.issues as issue (issue.code + issue.message)}
        <p class="issue {issue.severity}">{issue.message}</p>
      {/each}
    </div>
    <div class="side">
      <Badge tone={badge.tone} title={badge.hint}>{badge.label}</Badge>
      {#if info?.behavior === 'uncertain'}
        <Badge title="Allowlisted, but Roblox hasn't documented exactly what it does.">Effect not documented</Badge>
      {/if}
    </div>
    <div class="editor">
      {#if info?.input.kind === 'toggle' || (!info && kind === 'bool')}
        <Segmented
          size="sm"
          label="{info?.title ?? row.name} value"
          value={boolValue}
          options={[
            { value: 'default', label: 'Default', title: 'Not set: Roblox decides' },
            { value: 'True', label: 'On' },
            { value: 'False', label: 'Off' },
          ]}
          onchange={(v) => onset(row.name, v === 'default' ? null : v)}
        />
      {:else if info?.input.kind === 'choice'}
        <div class="choice-wrap">
          <select
            class="choice"
            aria-label="{info.title} value"
            value={row.value ?? ''}
            onchange={(e) => onset(row.name, (e.currentTarget as HTMLSelectElement).value || null)}
          >
            <option value="">Default</option>
            {#each info.input.options as o (o.value)}<option value={o.value}>{o.label}</option>{/each}
            {#if row.value && !info.input.options.some((o) => o.value === row.value)}<option value={row.value}
                >{row.value} (custom)</option
              >{/if}
          </select>
          <ChevronDown size={14} strokeWidth={2} aria-hidden="true" />
        </div>
      {:else}
        <div class="num">
          <input
            class="mono"
            class:invalid={!!draftError}
            aria-label="{info?.title ?? row.name} value"
            placeholder="Default"
            inputmode={kind === 'int' ? 'numeric' : 'text'}
            bind:value={draft}
            onblur={commitDraft}
            onkeydown={(e) => e.key === 'Enter' && commitDraft()}
          />
          {#if row.value !== null}
            <Button
              size="sm"
              variant="ghost"
              icon={Trash}
              label="Remove {row.name}"
              title={info ? 'Back to default' : 'Remove'}
              onclick={() => onset(row.name, null)}
            />
          {/if}
        </div>
      {/if}
      {#if row.custom && kind === 'bool'}
        <Button
          size="sm"
          variant="ghost"
          icon={Trash}
          label="Remove {row.name}"
          title="Remove"
          onclick={() => onset(row.name, null)}
        />
      {/if}
    </div>
  </div>
  {#if draftError}<p class="draft-error">{draftError}</p>{/if}

  {#if open}
    <div class="details" transition:slide={{ duration: 180 }}>
      {#if info}
        <p>{info.details}</p>
        {#if info.input.kind === 'number'}<p class="faint">Usual range {info.input.min}–{info.input.max}.</p>{/if}
        <p class="faint">{defaultText}</p>
        {#each info.requires as r (r.flag)}<p class="faint">
            Needs <span class="mono">{r.flag}</span> = {r.when}. {r.reason}
          </p>{/each}
        {#each info.conflicts as c (c.flag)}<p class="faint">
            Conflicts with <span class="mono">{c.flag}</span>. {c.reason}
          </p>{/each}
        {#if info.observations.length}
          {#each info.observations as o (o.robloxVersion + o.date)}
            <p class="faint">Seen {o.result} on Roblox {o.robloxVersion} ({o.date}): {o.method}</p>
          {/each}
        {/if}
        <div class="refs">
          {#each info.references as ref (ref)}
            {@const s = catalog.sources[ref]}
            {#if s}
              <button class="ref" onclick={() => api.openUrl(s.url)}
                >{s.label} <ExternalLink size={12} aria-hidden="true" /></button
              >
            {/if}
          {/each}
        </div>
        <p class="faint xs">
          Behaviour {info.behavior === 'established'
            ? 'is consistent across independent sources'
            : "isn't documented by Roblox"} · catalog reviewed {catalog.reviewed}
        </p>
      {:else}
        <p>
          This flag isn't on Roblox's allowlist. Since September 2025 the Roblox client ignores local flags like it and
          logs “Denied local configuration”. Vesper keeps it in the profile so you can see what Roblox says after a
          launch.
        </p>
      {/if}
    </div>
  {/if}
</div>

<style>
  .row {
    border-top: 1px solid var(--border);
    padding: var(--space-4) var(--space-4) var(--space-4) var(--space-2);
    transition: background var(--dur-fast);
  }
  .row:first-child {
    border-top: 0;
  }
  .row.set {
    background: linear-gradient(90deg, color-mix(in srgb, var(--accent) 7%, transparent), transparent 40%);
  }
  .main {
    display: grid;
    grid-template-columns: 28px 1fr auto auto;
    gap: var(--space-3);
    align-items: start;
  }
  .expand {
    width: 26px;
    height: 26px;
    display: grid;
    place-items: center;
    border: 0;
    border-radius: 7px;
    background: transparent;
    color: var(--text-3);
    cursor: pointer;
    transition: transform var(--dur) var(--ease-out);
  }
  .expand:hover {
    background: var(--surface-2);
    color: var(--text);
  }
  .open .expand {
    transform: rotate(180deg);
  }
  .title {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 8px;
    font-weight: 600;
  }
  .name {
    margin-top: 2px;
    color: var(--text-3);
    font-size: var(--text-xs);
    user-select: all;
  }
  .summary {
    margin-top: 6px;
    color: var(--text-2);
    font-size: var(--text-sm);
    max-width: 70ch;
  }
  .summary.bad,
  .issue.error {
    color: var(--bad);
  }
  .issue {
    margin-top: 6px;
    font-size: var(--text-sm);
    color: var(--warn);
  }
  .issue.info {
    color: var(--info);
  }
  .side {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 6px;
    padding-top: 2px;
  }
  .editor {
    display: flex;
    align-items: center;
    gap: 4px;
    min-width: 200px;
    justify-content: flex-end;
  }
  .choice,
  .num input {
    height: 30px;
    border-radius: 9px;
    border: 1px solid var(--border);
    background: var(--surface-2);
    padding: 0 10px;
    font-size: var(--text-sm);
  }
  .choice-wrap {
    position: relative;
    display: inline-flex;
    align-items: center;
  }
  .choice {
    appearance: none;
    min-width: 120px;
    padding-right: 30px;
    color: var(--text);
    cursor: pointer;
  }
  .choice:hover {
    border-color: var(--border-strong);
  }
  .choice:focus-visible {
    box-shadow: var(--focus);
  }
  .choice-wrap :global(svg) {
    position: absolute;
    right: 10px;
    pointer-events: none;
    color: var(--text-3);
  }
  .choice option {
    background: var(--surface-solid);
    color: var(--text);
  }
  .num {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .num input {
    width: 120px;
  }
  .num input.invalid {
    border-color: var(--bad);
  }
  .draft-error {
    margin: 6px 0 0 40px;
    color: var(--bad);
    font-size: var(--text-sm);
  }
  .details {
    margin: var(--space-3) 0 0 40px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: var(--text-sm);
    color: var(--text-2);
    max-width: 78ch;
  }
  .refs {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 4px;
  }
  .ref {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    height: 24px;
    padding: 0 9px;
    border-radius: 999px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text-2);
    font-size: var(--text-xs);
    cursor: pointer;
  }
  .ref:hover {
    color: var(--text);
    border-color: var(--border-strong);
  }
  @media (max-width: 1060px) {
    .main {
      grid-template-columns: 28px 1fr;
    }
    .side,
    .editor {
      grid-column: 2;
      align-items: flex-start;
      justify-content: flex-start;
    }
  }
</style>
