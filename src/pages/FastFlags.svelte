<script lang="ts">
  import { Download, ExternalLink, ClipboardPaste, Plus, RefreshCw, Search, Upload, Wand } from '@lucide/svelte';
  import { open, save } from '@tauri-apps/plugin-dialog';
  import { onMount } from 'svelte';
  import FlagRow from '../components/flags/FlagRow.svelte';
  import Badge from '../components/ui/Badge.svelte';
  import Button from '../components/ui/Button.svelte';
  import Dialog from '../components/ui/Dialog.svelte';
  import Segmented from '../components/ui/Segmented.svelte';
  import Select from '../components/ui/Select.svelte';
  import { api } from '../lib/api';
  import {
    buildRows,
    CATEGORY_LABELS,
    categoryCounts,
    filterRows,
    normalizeValue,
    parseFlagJson,
    type Show,
  } from '../lib/flags';
  import { relativeTime } from '../lib/format';
  import { flagsTarget } from '../lib/selection.svelte';
  import { isMac, modKey } from '../lib/platform';
  import { app } from '../lib/store.svelte';
  import type { FlagSet, InstalledFlags, Issue, PublishedValues } from '../lib/types';

  const catalog = $derived(app.boot!.catalog);
  const profileId = $derived(
    flagsTarget.id && app.profiles.profiles.some((p) => p.id === flagsTarget.id) ? flagsTarget.id : app.profiles.active,
  );
  const profile = $derived(app.profiles.profiles.find((p) => p.id === profileId)!);

  let query = $state('');
  let category = $state<string | null>(null);
  let show = $state<Show>('all');
  let issues = $state<Issue[]>([]);
  let published = $state<PublishedValues | null>(app.boot?.published ?? null);
  let installed = $state<InstalledFlags | null>(null);
  let searchEl = $state<HTMLInputElement | null>(null);

  let addOpen = $state(false);
  let addName = $state('');
  let addValue = $state('');
  let addError = $state<string | null>(null);
  let pasteOpen = $state(false);
  let pasteText = $state('');
  let pasteError = $state<string | null>(null);

  onMount(() => {
    api
      .flagsPublished(false)
      .then((p) => (published = p))
      .catch(() => {});
    api
      .flagsInstalled()
      .then((i) => (installed = i))
      .catch(() => {});
  });

  $effect(() => {
    const flags = profile?.flags;
    if (!flags) return;
    api
      .flagsValidate(flags)
      .then((r) => (issues = r))
      .catch(() => (issues = []));
  });

  const rows = $derived(buildRows(catalog, profile?.flags ?? {}, app.boot?.verification.flags ?? {}, issues));
  const visible = $derived(filterRows(rows, query, category, show));
  const counts = $derived(categoryCounts(rows));
  const setCount = $derived(Object.keys(profile?.flags ?? {}).length);
  const problems = $derived(issues.filter((i) => i.severity !== 'info'));

  async function setFlags(flags: FlagSet) {
    const store = await app.attempt(() => api.profilesUpdate(profile.id, { flags }));
    app.setProfiles(store);
  }

  function setFlag(name: string, value: string | null) {
    const next = { ...profile.flags };
    if (value === null) delete next[name];
    else next[name] = value;
    // Picking one graphics API turns the others off, so they can't conflict.
    const apis = ['FFlagDebugGraphicsPreferVulkan', 'FFlagDebugGraphicsPreferOpenGL', 'FFlagDebugGraphicsPreferD3D11'];
    if (value === 'True' && apis.includes(name)) for (const other of apis) if (other !== name) delete next[other];
    // Setting a texture level enables the override it depends on.
    if (name === 'DFIntTextureQualityOverride' && value !== null) next.DFFlagTextureQualityOverrideEnabled = 'True';
    setFlags(next);
  }

  function applyFix(issue: Issue) {
    if (!issue.fix) return;
    if (issue.fix.kind === 'remove') setFlag(issue.fix.flag, null);
    else setFlag(issue.fix.flag, issue.fix.value);
  }

  function addCustom() {
    const name = addName.trim();
    const r = normalizeValue(name, addValue);
    if (r.error) {
      addError = r.error;
      return;
    }
    setFlag(name, r.value!);
    addOpen = false;
    addName = '';
    addValue = '';
    addError = null;
  }

  async function importJson() {
    const path = await open({ multiple: false, filters: [{ name: 'Flags JSON', extensions: ['json'] }] });
    if (typeof path !== 'string') return;
    const r = await app.attempt(() => api.profilesImport(path));
    if (r) {
      app.setProfiles(r.store);
      flagsTarget.id = r.profile.id;
      app.toast(`Imported as a new profile, “${r.profile.name}”. Your other profiles weren't changed.`, 'ok');
    }
  }

  function mergePasted() {
    const r = parseFlagJson(pasteText);
    if (r.error) {
      pasteError = r.error;
      return;
    }
    setFlags({ ...profile.flags, ...r.flags });
    pasteOpen = false;
    pasteText = '';
    pasteError = null;
  }

  async function exportJson() {
    const path = await save({
      defaultPath: `${profile.name}.vesper-profile.json`,
      filters: [{ name: 'Vesper profile', extensions: ['json'] }],
    });
    if (!path) return;
    await app.attempt(() => api.profilesExport(profile.id, path), 'Exported this profile');
  }

  async function copyJson() {
    await navigator.clipboard.writeText(JSON.stringify(profile.flags, null, 2));
    app.toast('Copied as ClientAppSettings.json', 'ok');
  }

  async function refreshPublished() {
    published = (await app.attempt(() => api.flagsPublished(true))) ?? published;
  }

  function onkeydown(e: KeyboardEvent) {
    if ((isMac ? e.metaKey : e.ctrlKey) && e.key.toLowerCase() === 'f') {
      e.preventDefault();
      searchEl?.focus();
    }
  }
</script>

<svelte:window {onkeydown} />

<header class="head">
  <div>
    <p class="eyebrow">Fast Flags</p>
    <h1>Only what Roblox accepts.</h1>
  </div>
  <div class="row">
    <span class="faint small">Editing</span>
    <Select
      label="Profile to edit"
      value={profileId}
      options={app.profiles.profiles.map((p) => ({
        value: p.id,
        label: p.id === app.profiles.active ? `${p.name} (active)` : p.name,
      }))}
      onchange={(id) => (flagsTarget.id = id)}
    />
  </div>
</header>

<section class="banner glass">
  <div class="banner-copy">
    <p>
      <strong
        >Since {new Date(catalog.allowlist.effective).toLocaleDateString(undefined, {
          day: 'numeric',
          month: 'long',
          year: 'numeric',
        })}, Roblox only reads {catalog.flags.length} allowlisted Fast Flags.</strong
      >
      Everything else in ClientAppSettings.json is ignored. After each launch, Vesper reads Roblox's own log to confirm which
      flags it accepted.
    </p>
    <p class="faint small">
      Roblox's current values {published ? `fetched ${relativeTime(published.fetchedAt)}` : 'not fetched yet'} · catalog reviewed
      {catalog.reviewed}
    </p>
  </div>
  <div class="row">
    <Button size="sm" variant="ghost" icon={RefreshCw} onclick={refreshPublished}>Refresh values</Button>
    <Button size="sm" variant="ghost" icon={ExternalLink} onclick={() => api.openUrl(catalog.allowlist.url)}
      >Roblox announcement</Button
    >
  </div>
</section>

{#if problems.length}
  <section class="issues glass">
    <h3>{problems.length} thing{problems.length === 1 ? '' : 's'} to look at</h3>
    {#each problems as issue (issue.flag + issue.code)}
      <div class="issue {issue.severity}">
        <span class="mono small">{issue.flag}</span>
        <span>{issue.message}</span>
        {#if issue.fix}
          <Button size="sm" icon={Wand} onclick={() => applyFix(issue)}>
            {issue.fix.kind === 'remove'
              ? `Remove ${issue.fix.flag === issue.flag ? 'it' : issue.fix.flag}`
              : `Set ${issue.fix.flag} to ${issue.fix.value}`}
          </Button>
        {/if}
      </div>
    {/each}
  </section>
{/if}

<div class="toolbar">
  <label class="search">
    <Search size={15} aria-hidden="true" />
    <input
      bind:this={searchEl}
      bind:value={query}
      placeholder="Search flags ({modKey}F)"
      aria-label="Search flags"
      spellcheck="false"
    />
  </label>
  <Segmented
    size="sm"
    label="Show"
    bind:value={show}
    options={[
      { value: 'all', label: 'All' },
      { value: 'set', label: `Set · ${setCount}` },
      { value: 'issues', label: 'Needs attention' },
    ]}
  />
  <span class="spacer"></span>
  <Button size="sm" icon={Plus} onclick={() => (addOpen = true)}>Custom flag</Button>
  <Button size="sm" variant="ghost" icon={ClipboardPaste} onclick={() => (pasteOpen = true)}>Paste JSON</Button>
  <Button
    size="sm"
    variant="ghost"
    icon={Upload}
    label="Import JSON as a profile"
    title="Import JSON as a new profile"
    onclick={importJson}
  />
  <Button size="sm" variant="ghost" icon={Download} label="Export JSON" title="Export" onclick={exportJson} />
</div>

<div class="cats" role="group" aria-label="Category">
  <button class:active={category === null} onclick={() => (category = null)}>All <span>{rows.length}</span></button>
  {#each Object.entries(CATEGORY_LABELS) as [key, label] (key)}
    <button class:active={category === key} class:empty={counts[key] === 0} onclick={() => (category = key)}>
      {label} <span>{counts[key]}</span>
    </button>
  {/each}
</div>

<section class="list glass">
  {#if category && counts[category] === 0}
    <div class="empty">
      <h3>No {CATEGORY_LABELS[category]} flags are allowed</h3>
      <p class="muted">
        Roblox's allowlist has no {CATEGORY_LABELS[category].toLowerCase()} flags, so local changes in this area are ignored
        by the client. Flags you may have seen in old guides for this category no longer do anything.
      </p>
    </div>
  {:else if visible.length === 0}
    <div class="empty">
      <h3>Nothing matches</h3>
      <p class="muted">Try another search or category.</p>
    </div>
  {:else}
    {#each visible as row (row.name)}
      <FlagRow {row} {catalog} published={published?.values[row.name] ?? null} onset={setFlag} />
    {/each}
  {/if}
</section>

<section class="installed glass">
  <div>
    <h3>In Roblox right now</h3>
    {#if !installed?.file}
      <p class="muted small">Roblox isn't installed, so there's no flags file to show.</p>
    {:else if installed.error}
      <p class="small bad">{installed.error}</p>
    {:else if installed.flags}
      <p class="muted small">
        {Object.keys(installed.flags).length} flag(s) in <span class="mono">{installed.file}</span>
        {installed.writtenByVesper ? '· written by Vesper' : '· not written by Vesper (kept as is)'}
      </p>
    {:else}
      <p class="muted small">
        No flags file. Roblox uses its defaults until Vesper launches it with a profile that sets flags.
      </p>
    {/if}
  </div>
  <div class="row">
    <Button size="sm" variant="ghost" onclick={copyJson}>Copy profile as JSON</Button>
    <Badge tone="info">Applied at launch</Badge>
  </div>
</section>

<Dialog
  bind:open={addOpen}
  title="Add a custom flag"
  description="Flags that aren't on Roblox's allowlist are ignored by Roblox. Vesper will show what Roblox says after your next launch."
>
  <div class="form">
    <input
      class="text mono"
      placeholder="FFlagExample"
      bind:value={addName}
      spellcheck="false"
      aria-label="Flag name"
    />
    <input
      class="text mono"
      placeholder="True, 100 or text"
      bind:value={addValue}
      spellcheck="false"
      aria-label="Flag value"
      onkeydown={(e) => e.key === 'Enter' && addCustom()}
    />
    {#if addError}<p class="small bad">{addError}</p>{/if}
  </div>
  {#snippet actions()}
    <Button variant="ghost" onclick={() => (addOpen = false)}>Cancel</Button>
    <Button variant="primary" disabled={!addName.trim()} onclick={addCustom}>Add flag</Button>
  {/snippet}
</Dialog>

<Dialog
  bind:open={pasteOpen}
  title="Paste flags"
  description="Paste a ClientAppSettings.json-style object. The flags are merged into “{profile?.name}”."
  width={560}
>
  <textarea
    class="text mono"
    rows="9"
    bind:value={pasteText}
    placeholder={'{\n  "FIntDebugForceMSAASamples": "4"\n}'}
    aria-label="Flags JSON"></textarea>
  {#if pasteError}<p class="small bad">{pasteError}</p>{/if}
  {#snippet actions()}
    <Button variant="ghost" onclick={() => (pasteOpen = false)}>Cancel</Button>
    <Button variant="primary" disabled={!pasteText.trim()} onclick={mergePasted}>Merge flags</Button>
  {/snippet}
</Dialog>

<style>
  .head {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: var(--space-4);
    margin: var(--space-2) 0 var(--space-5);
  }
  h1 {
    font-size: var(--text-3xl);
    margin-top: 4px;
  }
  .banner {
    display: flex;
    align-items: center;
    gap: var(--space-5);
    padding: var(--space-4) var(--space-5);
    border-color: color-mix(in srgb, var(--accent) 30%, var(--border));
  }
  .banner-copy {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .issues {
    margin-top: var(--space-4);
    padding: var(--space-4) var(--space-5);
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .issue {
    display: flex;
    align-items: center;
    gap: var(--space-3);
    flex-wrap: wrap;
    font-size: var(--text-sm);
    color: var(--warn);
  }
  .issue.error {
    color: var(--bad);
  }
  .issue span:nth-child(2) {
    flex: 1;
    min-width: 200px;
    color: var(--text-2);
  }
  .toolbar {
    display: flex;
    align-items: center;
    gap: var(--space-2);
    flex-wrap: wrap;
    margin: var(--space-5) 0 var(--space-3);
  }
  .search {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 34px;
    padding: 0 12px;
    width: min(320px, 100%);
    border-radius: var(--radius-md);
    border: 1px solid var(--border);
    background: var(--surface-2);
    color: var(--text-3);
  }
  .search:focus-within {
    border-color: var(--accent);
  }
  .search input {
    flex: 1;
    border: 0;
    background: transparent;
    color: var(--text);
  }
  .search input:focus-visible {
    box-shadow: none;
  }
  .cats {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-bottom: var(--space-3);
  }
  .cats button {
    height: 28px;
    padding: 0 11px;
    border-radius: 999px;
    border: 1px solid var(--border);
    background: transparent;
    color: var(--text-2);
    font-size: var(--text-sm);
    cursor: pointer;
  }
  .cats button span {
    margin-left: 4px;
    color: var(--text-3);
    font-size: var(--text-xs);
  }
  .cats button:hover {
    color: var(--text);
  }
  .cats button.active {
    color: var(--text);
    background: var(--surface-3);
    border-color: var(--border-strong);
  }
  .cats button.empty {
    opacity: 0.55;
  }
  .list {
    padding: var(--space-1) var(--space-2);
  }
  .empty {
    padding: var(--space-8) var(--space-5);
    text-align: center;
  }
  .empty p {
    margin: var(--space-2) auto 0;
    max-width: 56ch;
  }
  .installed {
    margin-top: var(--space-4);
    padding: var(--space-4) var(--space-5);
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--space-4);
    flex-wrap: wrap;
  }
  .installed h3 {
    margin-bottom: 4px;
  }
  .bad {
    color: var(--bad);
  }
  .form {
    display: flex;
    flex-direction: column;
    gap: var(--space-2);
  }
  .text {
    width: 100%;
    padding: 9px 12px;
    border-radius: var(--radius-md);
    border: 1px solid var(--border);
    background: var(--surface-2);
  }
  textarea.text {
    resize: vertical;
  }
</style>
