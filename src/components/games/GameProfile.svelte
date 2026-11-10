<script lang="ts">
  import Button from '../ui/Button.svelte';
  import Dialog from '../ui/Dialog.svelte';
  import Select from '../ui/Select.svelte';
  import { api } from '../../lib/api';
  import { gameKey, profileForGame } from '../../lib/games';
  import { app } from '../../lib/store.svelte';

  interface Props {
    open: boolean;
    placeId: number;
    universeId: number | null;
    name: string | null;
  }
  let { open = $bindable(), placeId, universeId, name }: Props = $props();

  const current = $derived(profileForGame(app.settings, placeId, universeId) ?? '');
  const options = $derived([
    { value: '', label: 'Whatever profile is active' },
    ...app.profiles.profiles.map((p) => ({ value: p.id, label: p.name })),
  ]);

  async function choose(value: string) {
    const s = await app.attempt(() => api.gamesProfileSet(gameKey(placeId, universeId), value || null));
    if (s) {
      app.settings = s;
      const chosen = app.profiles.profiles.find((p) => p.id === value);
      app.toast(
        chosen ? `${name ?? 'This game'} will launch with ${chosen.name}.` : 'Profile cleared for this game.',
        'ok',
      );
    }
  }
</script>

<Dialog
  bind:open
  title="Profile for this game"
  description={`When you join ${name ?? 'this game'} from Vesper, it launches with the profile you pick here. Your active profile stays as it is.`}
>
  <Select label="Profile" value={current} {options} onchange={choose} />
  <p class="note">Applies to joins from Vesper: Home, Recent games, the tray menu and “Join server” links.</p>
  {#snippet actions()}
    <Button onclick={() => (open = false)}>Done</Button>
  {/snippet}
</Dialog>

<style>
  .note {
    margin: var(--space-3) 0 0;
    font-size: var(--text-sm);
    color: var(--text-3);
  }
</style>
