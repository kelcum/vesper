<script lang="ts">
  import { Copy, Download, Pencil, Plus, RotateCcw, Star, Trash, Upload, Check } from '@lucide/svelte';
  import { open, save } from '@tauri-apps/plugin-dialog';
  import FpsCapPicker from '../components/profiles/FpsCapPicker.svelte';
  import Badge from '../components/ui/Badge.svelte';
  import Button from '../components/ui/Button.svelte';
  import Dialog from '../components/ui/Dialog.svelte';
  import Field from '../components/ui/Field.svelte';
  import Segmented from '../components/ui/Segmented.svelte';
  import { api } from '../lib/api';
  import { fpsCapShort, priorityLabel } from '../lib/format';
  import { app } from '../lib/store.svelte';
  import type { FpsCap, Priority, Profile } from '../lib/types';
  import { flagsTarget } from '../lib/selection.svelte';

  const displayHz = $derived(app.boot?.system.displayHz ?? null);
  const cpuCount = $derived(app.boot?.system.logicalProcessors ?? 1);

  let editing = $state<Profile | null>(null);
  let editorOpen = $state(false);
  let name = $state('');
  let description = $state('');
  let creating = $state(false);
  let newName = $state('');
  let confirmDelete = $state<Profile | null>(null);
  let confirmOpen = $state(false);

  function edit(p: Profile) {
    editing = p;
    name = p.name;
    description = p.description;
    editorOpen = true;
  }

  async function patch(p: Partial<{ name: string; description: string; fpsCap: FpsCap | null; priority: Priority; affinity: number | null }>) {
    if (!editing) return;
    const store = await app.attempt(() => api.profilesUpdate(editing!.id, p));
    if (store) {
      app.setProfiles(store);
      editing = store.profiles.find((x) => x.id === editing!.id) ?? null;
    }
  }

  async function saveText() {
    if (!editing) return;
    if (name.trim() !== editing.name || description !== editing.description) await patch({ name, description });
  }

  async function exportProfile(p: Profile) {
    const path = await save({ defaultPath: `${p.name}.vesper-profile.json`, filters: [{ name: 'Vesper profile', extensions: ['json'] }] });
    if (path) await app.attempt(() => api.profilesExport(p.id, path), `Exported “${p.name}”`);
  }

  async function importProfile() {
    const path = await open({ multiple: false, filters: [{ name: 'Profile or flags JSON', extensions: ['json'] }] });
    if (typeof path !== 'string') return;
    const r = await app.attempt(() => api.profilesImport(path));
    if (r) {
      app.setProfiles(r.store);
      const ignored = r.issues.filter((i) => i.code === 'not_allowlisted').length;
      app.toast(
        ignored
          ? `Imported “${r.profile.name}”. ${ignored} flag(s) aren't on Roblox's allowlist and will be ignored.`
          : `Imported “${r.profile.name}”.`,
        ignored ? 'warn' : 'ok',
      );
    }
  }

  async function create() {
    const store = await app.attempt(() => api.profilesCreate(newName));
    if (store) {
      app.setProfiles(store);
      creating = false;
      const created = store.profiles[store.profiles.length - 1];
      newName = '';
      edit(created);
    }
  }

  async function remove() {
    if (!confirmDelete) return;
    const id = confirmDelete.id;
    confirmOpen = false;
    app.setProfiles(await app.attempt(() => api.profilesDelete(id), 'Profile deleted'));
  }

  // BigInt: JavaScript's bitwise operators are 32-bit, and PCs can have more processors.
  function toggleCore(i: number) {
    if (!editing) return;
    const all = (1n << BigInt(Math.min(cpuCount, 64))) - 1n;
    const current = editing.affinity == null ? all : BigInt(editing.affinity);
    const next = current ^ (1n << BigInt(i));
    if (next === 0n) {
      app.toast('Roblox needs at least one processor.', 'warn');
      return;
    }
    patch({ affinity: next === all ? null : Number(next) });
  }

  function coreOn(i: number) {
    return editing?.affinity == null || ((BigInt(editing.affinity) >> BigInt(i)) & 1n) === 1n;
  }

  function editFlags(p: Profile) {
    flagsTarget.id = p.id;
    editorOpen = false;
    app.go('flags');
  }
</script>

<header class="head">
  <div>
    <p class="eyebrow">Profiles</p>
    <h1>Tune Roblox for the moment.</h1>
    <p class="muted lede">A profile bundles Fast Flags, a frame-rate limit and process options. The active one is applied each time Vesper launches Roblox.</p>
  </div>
  <div class="row">
    <Button icon={Upload} onclick={importProfile}>Import</Button>
    <Button variant="primary" icon={Plus} onclick={() => (creating = true)}>New profile</Button>
  </div>
</header>

<div class="grid">
  {#each app.profiles.profiles as p (p.id)}
    {@const active = p.id === app.profiles.active}
    {@const isDefault = p.id === app.profiles.default}
    {@const n = Object.keys(p.flags).length}
    <article class="profile glass" class:active>
      <div class="top">
        <h3>{p.name}</h3>
        {#if isDefault}<span class="star" title="Default profile"><Star size={14} fill="currentColor" aria-label="Default" /></span>{/if}
        <span class="spacer"></span>
        {#if active}<Badge tone="accent" dot>Active</Badge>{:else if p.builtin}<Badge>Built-in</Badge>{/if}
      </div>
      <p class="desc">{p.description || 'No description.'}</p>
      <div class="chips">
        <Badge>{fpsCapShort(p.fpsCap, displayHz)}</Badge>
        <Badge>{n} flag{n === 1 ? '' : 's'}</Badge>
        {#if p.priority !== 'normal'}<Badge>{priorityLabel(p.priority)}</Badge>{/if}
        {#if p.affinity != null}<Badge tone="warn">Custom affinity</Badge>{/if}
      </div>
      <div class="actions">
        {#if !active}
          <Button size="sm" icon={Check} onclick={() => app.attempt(() => api.profilesSetActive(p.id)).then((s) => app.setProfiles(s))}>Use</Button>
        {/if}
        <Button size="sm" icon={Pencil} onclick={() => edit(p)}>Edit</Button>
        <span class="spacer"></span>
        <Button size="sm" variant="ghost" icon={Copy} label="Duplicate" title="Duplicate" onclick={() => app.attempt(() => api.profilesDuplicate(p.id)).then((s) => app.setProfiles(s))} />
        <Button size="sm" variant="ghost" icon={Download} label="Export" title="Export" onclick={() => exportProfile(p)} />
        {#if !isDefault}
          <Button size="sm" variant="ghost" icon={Star} label="Make default" title="Make default" onclick={() => app.attempt(() => api.profilesSetDefault(p.id)).then((s) => app.setProfiles(s))} />
        {/if}
        {#if p.builtin}
          <Button size="sm" variant="ghost" icon={RotateCcw} label="Reset to built-in" title="Reset to built-in settings" onclick={() => app.attempt(() => api.profilesReset(p.id), `Reset “${p.name}”`).then((s) => app.setProfiles(s))} />
        {:else}
          <Button size="sm" variant="ghost" icon={Trash} label="Delete" title="Delete" onclick={() => { confirmDelete = p; confirmOpen = true; }} />
        {/if}
      </div>
    </article>
  {/each}
</div>

<Dialog bind:open={creating} title="New profile" description="Starts with Roblox's defaults. You can copy an existing profile with Duplicate instead.">
  <input class="text" placeholder="Profile name" bind:value={newName} maxlength="40" onkeydown={(e) => e.key === 'Enter' && newName.trim() && create()} />
  {#snippet actions()}
    <Button variant="ghost" onclick={() => (creating = false)}>Cancel</Button>
    <Button variant="primary" disabled={!newName.trim()} onclick={create}>Create</Button>
  {/snippet}
</Dialog>

<Dialog bind:open={confirmOpen} title="Delete “{confirmDelete?.name}”?" description="This can't be undone, but your backups still contain it.">
  {#snippet actions()}
    <Button variant="ghost" onclick={() => (confirmOpen = false)}>Cancel</Button>
    <Button variant="danger" onclick={remove}>Delete profile</Button>
  {/snippet}
</Dialog>

<Dialog bind:open={editorOpen} title="Edit profile" width={680} onclose={saveText}>
  {#if editing}
    <div class="editor">
      <Field label="Name" stacked>
        <input class="text" bind:value={name} maxlength="40" onblur={saveText} />
      </Field>
      <Field label="Description" stacked>
        <textarea class="text" rows="2" bind:value={description} maxlength="400" onblur={saveText}></textarea>
      </Field>
      <Field label="Frame-rate limit" stacked>
        <FpsCapPicker value={editing.fpsCap} {displayHz} onchange={(c) => patch({ fpsCap: c })} />
      </Field>
      <Field
        label="Process priority"
        hint="How Windows schedules Roblox against other programs. Applies when Vesper starts Roblox."
        warning={editing.priority === 'high' ? 'High priority can make other apps, voice chat and streaming stutter.' : undefined}
      >
        <Segmented
          label="Process priority"
          value={editing.priority}
          options={[
            { value: 'normal', label: 'Normal' },
            { value: 'aboveNormal', label: 'Above normal' },
            { value: 'high', label: 'High' },
          ]}
          onchange={(v) => patch({ priority: v })}
        />
      </Field>
      <Field label="Fast Flags" hint="{Object.keys(editing.flags).length} set in this profile.">
        <Button icon={Pencil} onclick={() => editFlags(editing!)}>Edit flags</Button>
      </Field>
      <details class="advanced">
        <summary>Advanced: processor affinity</summary>
        <p class="faint small">
          Restricts Roblox to the processors you pick. This usually makes performance worse; only change it if you know why you need it.
          Windows or Roblox's anti-cheat may refuse the change, and Vesper will tell you if that happens.
        </p>
        <div class="cores">
          {#each Array.from({ length: Math.min(cpuCount, 64) }, (_, i) => i) as i (i)}
            <button class="core" class:on={coreOn(i)} aria-pressed={coreOn(i)} onclick={() => toggleCore(i)}>{i}</button>
          {/each}
        </div>
        {#if editing.affinity != null}
          <Button size="sm" variant="ghost" onclick={() => patch({ affinity: null })}>Use all processors</Button>
        {/if}
      </details>
    </div>
  {/if}
  {#snippet actions()}
    <Button variant="primary" onclick={() => { saveText(); editorOpen = false; }}>Done</Button>
  {/snippet}
</Dialog>

<style>
  .head {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: var(--space-4);
    margin: var(--space-2) 0 var(--space-6);
  }
  h1 {
    font-size: var(--text-3xl);
    margin-top: 4px;
  }
  .lede {
    margin-top: var(--space-2);
    max-width: 60ch;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: var(--space-4);
  }
  .profile {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
    padding: var(--space-5);
    transition:
      border-color var(--dur) var(--ease-out),
      box-shadow var(--dur) var(--ease-out),
      transform var(--dur) var(--ease-out);
  }
  .profile:hover {
    border-color: var(--border-strong);
  }
  .profile.active {
    border-color: color-mix(in srgb, var(--accent) 55%, transparent);
    box-shadow:
      inset 0 1px 0 var(--highlight),
      var(--accent-glow);
  }
  .top {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .star {
    color: var(--warn);
    display: inline-flex;
  }
  .desc {
    color: var(--text-2);
    font-size: var(--text-sm);
    min-height: 3.6em;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-top: auto;
    padding-top: var(--space-2);
  }
  .text {
    width: 100%;
    padding: 9px 12px;
    border-radius: var(--radius-md);
    border: 1px solid var(--border);
    background: var(--surface-2);
    resize: vertical;
  }
  .text:focus-visible {
    border-color: var(--accent);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--accent) 22%, transparent);
  }
  .editor {
    display: flex;
    flex-direction: column;
  }
  .advanced {
    margin-top: var(--space-4);
    padding-top: var(--space-4);
    border-top: 1px solid var(--border);
  }
  summary {
    cursor: pointer;
    font-weight: 550;
    margin-bottom: var(--space-3);
  }
  .cores {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    margin: var(--space-3) 0;
  }
  .core {
    width: 34px;
    height: 28px;
    border-radius: 7px;
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text-3);
    font-family: var(--font-mono);
    font-size: var(--text-xs);
    cursor: pointer;
  }
  .core.on {
    color: var(--text);
    background: color-mix(in srgb, var(--accent) 18%, transparent);
    border-color: color-mix(in srgb, var(--accent) 50%, transparent);
  }
</style>
