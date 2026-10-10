import { mount } from 'svelte';
import './styles/tokens.css';
import './styles/base.css';
import App from './App.svelte';
import { app } from './lib/store.svelte';

// The context menu offers nothing useful in an app window (keep it in text fields).
window.addEventListener('contextmenu', (e) => {
  const t = e.target as HTMLElement | null;
  if (!t?.closest('input, textarea, [contenteditable="true"]')) e.preventDefault();
});

mount(App, { target: document.getElementById('app')! });
app.init();
