<script lang="ts">
  import { onMount } from 'svelte';
  import Select from '../ui/Select.svelte';
  import { api } from '../../lib/api';
  import { app } from '../../lib/store.svelte';
  import type { Option } from '../../lib/types';

  let regions = $state<Option[]>([]);
  onMount(async () => {
    regions = (await api.serversRegions().catch(() => [])) ?? [];
  });

  const servers = $derived(app.settings.servers);
  const cityLabel = (key: string) => key.split(':').slice(2).join(':');

  const value = $derived(servers.pick === 'region' ? `region:${servers.region}` : servers.pick);
  const options = $derived.by(() => {
    const list = [
      { value: 'off', label: 'Roblox picks the server' },
      { value: 'lowestPing', label: 'Lowest-ping server' },
      ...regions.map((r) => ({ value: `region:${r.id}`, label: `Server in ${r.label}` })),
    ];
    if (servers.region.startsWith('city:') && !list.some((o) => o.value === `region:${servers.region}`)) {
      list.push({ value: `region:${servers.region}`, label: `Server in ${cityLabel(servers.region)}` });
    }
    return list;
  });

  function choose(v: string) {
    if (v.startsWith('region:')) app.saveSettings({ servers: { pick: 'region', region: v.slice(7) } });
    else app.saveSettings({ servers: { pick: v as 'off' | 'lowestPing' } });
  }
</script>

<Select label="Which server to join" {value} {options} onchange={choose} />
