// Cross-page selection: which profile the Fast Flags page is editing.
// `null` means "the active profile".
export const flagsTarget = $state<{ id: string | null }>({ id: null });
