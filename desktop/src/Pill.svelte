<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'

  import { onMount } from 'svelte'

  let stopping = $state(false)
  let label = $state('')
  onMount(() => {
    invoke<string>('pill_info').then((s) => (label = s)).catch(() => {})
  })
  // Klavye: hap odaktayken Esc veya Ctrl+Shift+S paylaşımı durdurur.
  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape' || (e.ctrlKey && e.shiftKey && e.key.toLowerCase() === 's')) {
      e.preventDefault()
      stop()
    }
  }
  async function stop() {
    stopping = true
    try {
      await invoke('stop_screen_share')
    } catch {
      stopping = false
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="pill" data-tauri-drag-region role="status" aria-live="polite" aria-keyshortcuts="Escape Control+Shift+S">
  <span class="dot" aria-hidden="true"></span>
  <span class="t" data-tauri-drag-region>Ekran paylaşılıyor{label ? ` · ${label}` : ''}</span>
  <span class="sep" aria-hidden="true">·</span>
  <button class="stop" onclick={stop} disabled={stopping} title="Durdur (Esc veya Ctrl+Shift+S)">{stopping ? 'Durduruluyor…' : 'Durdur'}</button>
</div>

<style>
  :global(html),
  :global(body) {
    background: transparent !important;
    overflow: hidden;
  }
  .pill {
    box-sizing: border-box;
    height: 100vh;
    width: 100vw;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 8px 0 16px;
    border-radius: 99px;
    background: var(--s1);
    border: 1px solid var(--gold-line);
    color: var(--gold-hi);
    font-size: 13px;
    font-weight: 600;
    user-select: none;
    cursor: grab;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--gold-grad);
    animation: beat 1.8s ease-in-out infinite;
  }
  @keyframes beat {
    50% {
      transform: scale(1.5);
      opacity: 0.6;
    }
  }
  .t {
    flex: 1;
    white-space: nowrap;
  }
  .sep {
    color: var(--faint);
  }
  .stop {
    height: 28px;
    padding: 0 14px;
    border-radius: 99px;
    border: 0;
    font-weight: 700;
    background: var(--gold-grad);
    color: var(--on-gold);
    cursor: pointer;
    transition: opacity 150ms var(--ease);
  }
  .stop:hover:not(:disabled) {
    opacity: 0.9;
  }
  .stop:disabled {
    opacity: 0.6;
  }
  @media (prefers-reduced-motion: reduce) {
    .dot {
      animation: none;
    }
  }
</style>
