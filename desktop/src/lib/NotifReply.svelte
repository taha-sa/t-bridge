<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  import { app } from './store.svelte'
  import type { NotificationData } from './types'
  import Icon from './Icon.svelte'

  let { n, prominent = false }: { n: NotificationData; prominent?: boolean } = $props()

  const info = $derived(app.reply[n.key])
  const result = $derived(app.replyState[n.key])

  let open = $state(false)
  let text = $state('')
  let phase = $state<'idle' | 'sending' | 'sent' | 'error'>('idle')
  let err = $state('')
  let ta = $state<HTMLTextAreaElement>()
  let guard: ReturnType<typeof setTimeout> | undefined
  let doneTimer: ReturnType<typeof setTimeout> | undefined

  const LINE = 20
  const MAX_LINES = 4

  function grow() {
    if (!ta) return
    ta.style.height = 'auto'
    ta.style.height = Math.min(ta.scrollHeight, LINE * MAX_LINES + 4) + 'px'
  }

  function expand() {
    open = true
    queueMicrotask(() => {
      ta?.focus()
      grow()
    })
  }

  function collapse() {
    if (phase === 'sending') return
    open = false
    if (phase === 'error') phase = 'idle'
  }

  async function send(value: string = text) {
    const t = value.trim()
    if (!t || phase === 'sending') return
    phase = 'sending'
    err = ''
    delete app.replyState[n.key]
    clearTimeout(guard)
    // Telefon 15 sn içinde sonuç dönmezse hata say (metin korunur).
    guard = setTimeout(() => {
      if (phase === 'sending') {
        phase = 'error'
        err = 'Telefon yanıt vermedi'
      }
    }, 15000)
    try {
      await invoke('reply_notification', { key: n.key, text: t })
    } catch (e) {
      clearTimeout(guard)
      phase = 'error'
      err = String(e)
    }
  }

  // Telefondan gelen sonuç.
  $effect(() => {
    const r = result
    if (!r || phase !== 'sending') return
    clearTimeout(guard)
    if (r.ok) {
      phase = 'sent'
      text = ''
      open = false
      clearTimeout(doneTimer)
      doneTimer = setTimeout(() => (phase = 'idle'), 2000)
    } else {
      phase = 'error'
      err = r.error || 'Yanıt gönderilemedi'
    }
  })

  $effect(() => () => {
    clearTimeout(guard)
    clearTimeout(doneTimer)
  })

  function onkey(e: KeyboardEvent) {
    if (e.key === 'Enter' && !e.shiftKey && !e.isComposing) {
      e.preventDefault()
      send()
    } else if (e.key === 'Escape') {
      e.preventDefault()
      collapse()
    }
  }
</script>

{#if info?.canReply}
  <div class="reply" class:prominent>
    {#if phase === 'sent'}
      <div class="sent" role="status"><Icon name="check" size={15} /> Gönderildi</div>
    {:else if !open}
      {#if prominent}
        <button class="fake" onclick={expand} aria-label="Yanıtla">
          <span class="ph">Yanıt yaz…</span>
          <span class="go"><Icon name="up" size={14} /></span>
        </button>
      {:else}
        <button class="ghost sm" onclick={expand}>Yanıtla</button>
      {/if}
    {/if}

    <div class="panel" class:open aria-hidden={!open}>
      <div class="inner">
        {#if info.choices.length}
          <div class="choices">
            {#each info.choices as c (c)}
              <button class="chipbtn" disabled={phase === 'sending'} tabindex={open ? 0 : -1} onclick={() => send(c)}>{c}</button>
            {/each}
          </div>
        {/if}
        <div class="compose" class:bad={phase === 'error'}>
          <textarea
            bind:this={ta}
            bind:value={text}
            rows="1"
            placeholder="Yanıt yaz…"
            aria-label="Yanıt"
            disabled={phase === 'sending'}
            tabindex={open ? 0 : -1}
            oninput={grow}
            onkeydown={onkey}
          ></textarea>
          <button class="sendbtn" title="Gönder (Enter)" aria-label="Gönder" disabled={!text.trim() || phase === 'sending'} tabindex={open ? 0 : -1} onclick={() => send()}>
            {#if phase === 'sending'}<span class="spin" aria-hidden="true"></span>{:else}<Icon name="up" size={16} />{/if}
          </button>
        </div>
        {#if phase === 'error'}<div class="err small" role="alert">{err}</div>{/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .reply {
    margin-top: 8px;
  }
  .sent {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 10px;
    border-radius: 99px;
    font-size: 13px;
    font-weight: 600;
    color: var(--gold-hi);
    background: var(--gold-soft);
    border: 1px solid var(--gold-line);
    animation: pop 150ms var(--ease);
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: translateY(2px);
    }
  }
  .fake {
    width: 100%;
    max-width: 420px;
    height: 34px;
    padding: 0 6px 0 14px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    border-radius: 99px;
    background: var(--s1);
    border: 1px solid var(--gold-line);
    color: var(--muted);
    transition:
      border-color 150ms var(--ease),
      background 150ms var(--ease);
  }
  .fake:hover {
    background: var(--gold-soft);
  }
  .fake .go {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--gold-grad);
    color: var(--on-gold);
  }
  .ph {
    font-size: 13px;
  }
  /* Yükseklik animasyonu: düzen sıçramaz */
  .panel {
    display: grid;
    grid-template-rows: 0fr;
    opacity: 0;
    transition:
      grid-template-rows 150ms var(--ease),
      opacity 150ms var(--ease);
  }
  .panel.open {
    grid-template-rows: 1fr;
    opacity: 1;
  }
  .inner {
    overflow: hidden;
    min-height: 0;
  }
  .choices {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    padding: 2px 0 8px;
  }
  .chipbtn {
    height: 28px;
    padding: 0 12px;
    border-radius: 99px;
    font-size: 13px;
    background: var(--s3);
    border: 1px solid var(--line-2);
    color: var(--text-2);
  }
  .chipbtn:hover:not(:disabled) {
    border-color: var(--gold-line);
    color: var(--gold-hi);
    background: var(--gold-soft);
  }
  .compose {
    display: flex;
    align-items: flex-end;
    gap: 8px;
    padding: 6px 6px 6px 12px;
    background: var(--s1);
    border: 1px solid var(--line-2);
    border-radius: 14px;
    transition: border-color 150ms var(--ease);
  }
  .compose:focus-within {
    border-color: var(--gold-line);
  }
  .compose.bad {
    border-color: var(--bad);
  }
  textarea {
    flex: 1;
    min-width: 0;
    resize: none;
    border: 0;
    outline: 0;
    background: transparent;
    color: var(--text);
    font: inherit;
    line-height: 20px;
    padding: 4px 0;
    height: 28px;
    max-height: 84px;
  }
  textarea:disabled {
    opacity: 0.6;
  }
  .sendbtn {
    flex: none;
    width: 32px;
    height: 32px;
    padding: 0;
    border-radius: 50%;
    display: grid;
    place-items: center;
    border: 0;
    background: var(--gold-grad);
    color: var(--on-gold);
    transition: opacity 150ms var(--ease);
  }
  .sendbtn:disabled {
    opacity: 0.4;
  }
  .err {
    margin-top: 6px;
    color: var(--bad);
  }
  .spin {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    border: 2px solid rgba(26, 18, 4, 0.3);
    border-top-color: var(--on-gold);
    animation: sp 700ms linear infinite;
  }
  @keyframes sp {
    to {
      transform: rotate(360deg);
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .panel,
    .fake,
    .compose {
      transition: none;
    }
    .spin {
      animation-duration: 2s;
    }
  }
</style>
