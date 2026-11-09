<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { getCurrentWindow } from '@tauri-apps/api/window'
  import { onMount } from 'svelte'

  type Info = { id: number; name: string; timeout_secs: number }
  type Choice = 'allow' | 'always' | 'deny'

  let info = $state<Info | null>(null)
  let left = $state(60)
  let busy = $state(false)
  let denyBtn: HTMLButtonElement | undefined = $state()

  async function decide(choice: Choice) {
    if (busy || !info) return
    busy = true
    try {
      await invoke('consent_result', { id: info.id, choice })
    } catch {
      busy = false
    }
  }

  onMount(() => {
    let timer: ReturnType<typeof setInterval> | undefined
    invoke<Info | null>('consent_info').then((i) => {
      if (!i) {
        // Bekleyen istek yok (bayat pencere): kapat.
        getCurrentWindow().close()
        return
      }
      info = i
      left = i.timeout_secs
      const end = Date.now() + i.timeout_secs * 1000
      timer = setInterval(() => {
        left = Math.max(0, Math.ceil((end - Date.now()) / 1000))
      }, 250)
      // Güvenlik: varsayılan odak "Reddet" (yanlışlıkla Enter izin vermesin).
      denyBtn?.focus()
    })
    return () => timer && clearInterval(timer)
  })

  function onKey(e: KeyboardEvent) {
    if (e.key === 'Escape') {
      e.preventDefault()
      decide('deny')
    }
  }

  const pct = $derived(info ? Math.max(0, Math.min(100, (left / info.timeout_secs) * 100)) : 100)
</script>

<svelte:window onkeydown={onKey} />

<div class="wrap">
  {#if info}
    <div class="card" role="alertdialog" aria-modal="true" aria-labelledby="c-title" aria-describedby="c-body">
      <div class="head">
        <span class="ico" aria-hidden="true">
          <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
            <rect x="3" y="4" width="18" height="12" rx="2.2" />
            <path d="M8 20h8M12 16v4" />
          </svg>
        </span>
        <h2 id="c-title">Ekran paylaşımı isteği</h2>
      </div>

      <p id="c-body" class="body">
        <strong class="who">{info.name}</strong> ekranı görmek istiyor. İzin verirsen telefon bu bilgisayarın ekranını görür; fareyi ve klavyeyi kullanabilir.
      </p>

      <div class="timer" aria-hidden="true">
        <div class="bar"><span style="width: {pct}%"></span></div>
        <span class="num">{left} sn içinde yanıt verilmezse reddedilir</span>
      </div>

      <div class="actions">
        <button class="ghost always" onclick={() => decide('always')} disabled={busy} aria-label="Her zaman izin ver: bu telefona bir daha sorma">
          Her zaman izin ver
        </button>
        <span class="spacer"></span>
        <button bind:this={denyBtn} class="deny" onclick={() => decide('deny')} disabled={busy} aria-label="Reddet">Reddet</button>
        <button class="primary" onclick={() => decide('allow')} disabled={busy} aria-label="İzin ver">İzin ver</button>
      </div>
    </div>
  {/if}
</div>

<style>
  :global(html),
  :global(body) {
    background: transparent !important;
    overflow: hidden;
  }
  .wrap {
    box-sizing: border-box;
    height: 100vh;
    width: 100vw;
    padding: 12px; /* pencere saydam: gölge için pay */
  }
  .card {
    box-sizing: border-box;
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 18px 20px 16px;
    border-radius: 12px;
    background: var(--s1);
    border: 1px solid var(--gold-line);
    box-shadow: var(--shadow-lg);
    color: var(--text);
    animation: fade 150ms var(--ease) both;
  }
  @keyframes fade {
    from {
      opacity: 0;
      transform: translateY(4px);
    }
  }
  .head {
    display: flex;
    align-items: center;
    gap: 12px;
  }
  .ico {
    flex: none;
    width: 38px;
    height: 38px;
    border-radius: 10px;
    display: grid;
    place-items: center;
    color: var(--gold-hi);
    background: var(--gold-soft);
    border: 1px solid var(--gold-line);
  }
  h2 {
    font-size: 17px;
    color: var(--text);
  }
  .body {
    margin: 0;
    font-size: 14px;
    color: var(--text-2);
  }
  .who {
    color: var(--gold-hi);
    font-weight: 650;
    overflow-wrap: anywhere;
  }
  .timer {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin-top: auto;
  }
  .bar {
    height: 3px;
    border-radius: 99px;
    background: var(--line-2);
    overflow: hidden;
  }
  .bar span {
    display: block;
    height: 100%;
    background: var(--gold-grad);
    transition: width 250ms linear;
  }
  .num {
    font-size: 12px;
    color: var(--muted);
  }
  .actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .spacer {
    flex: 1;
  }
  .always {
    padding: 0 10px;
    color: var(--muted);
    font-size: 13px;
  }
  @media (prefers-reduced-motion: reduce) {
    .card {
      animation: none;
    }
    .bar span {
      transition: none;
    }
  }
</style>
