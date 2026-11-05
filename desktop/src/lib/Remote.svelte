<script lang="ts">
  import { app, call, flash } from './store.svelte'
  import type { MediaAction, PhoneCommand, VolumeAction } from './types'
  import Icon from './Icon.svelte'

  const s = $derived(app.s)
  const on = $derived(s.state === 'Connected')
  let ringing = $state(false)
  let ringTimer: ReturnType<typeof setTimeout> | undefined

  const send = (cmd: PhoneCommand) => call<boolean>('phone_cmd', { cmd })

  async function ring() {
    ringing = !ringing
    await send({ Ring: { on: ringing } })
    clearTimeout(ringTimer)
    if (ringing) {
      flash('Telefon çalıyor (30 sn)')
      ringTimer = setTimeout(() => (ringing = false), 30_000)
    }
  }
  $effect(() => {
    if (!on) ringing = false
  })
  const media = (action: MediaAction) => send({ Media: { action } })
  const vol = (action: VolumeAction) => send({ Volume: { action } })
</script>

<div class="page">
  {#if !on}
    <section class="card tight off">
      <Icon name="wifi" />
      <span class="muted">Telefon bağlı değil. Bağlanınca bu sayfadan telefonu kontrol edebilirsin.</span>
    </section>
  {/if}

  <section class="card find" class:ringing>
    <div class="pulse"><span></span><span></span><span></span></div>
    <div class="ringbtn">
      <button class="big" class:primary={!ringing} class:stop={ringing} disabled={!on} onclick={ring} aria-label={ringing ? 'Durdur' : 'Telefonu çaldır'}>
        <Icon name="ring" size={30} />
      </button>
    </div>
    <div class="grow">
      <div class="row">
        <h2>Telefonu bul</h2>
        {#if ringing}<span class="chip violet"><span class="dot"></span>çalıyor</span>{/if}
      </div>
      <p class="muted">Sessiz moddayken bile alarm sesiyle, tam seste 30 saniye çalar.</p>
      <button class="sm" disabled={!on} onclick={ring}>{ringing ? 'Durdur' : 'Çaldır'}</button>
    </div>
  </section>

  <section class="card media">
    <h3>Medya</h3>
    <div class="row np">
      <div class="art" class:playing={s.nowPlaying?.playing}>
        <Icon name="music" size={24} />
      </div>
      <div class="grow">
        {#if s.nowPlaying}
          <div class="tt ellipsis">{s.nowPlaying.title || 'Bilinmeyen parça'}</div>
          <div class="muted ellipsis small">{s.nowPlaying.artist}{s.nowPlaying.app_name ? ` · ${s.nowPlaying.app_name}` : ''}</div>
        {:else}
          <div class="tt muted">Şu an çalan bir şey yok</div>
          <div class="faint small">Kontroller yine de telefondaki son oynatıcıya gider.</div>
        {/if}
      </div>
    </div>
    <div class="controls">
      <button class="ghost icon lg" disabled={!on} onclick={() => media('Previous')} title="Önceki"><Icon name="prev" size={20} /></button>
      <button class="primary play" disabled={!on} onclick={() => media('PlayPause')} title="Oynat/Duraklat">
        <Icon name={s.nowPlaying?.playing ? 'pause' : 'play'} size={24} />
      </button>
      <button class="ghost icon lg" disabled={!on} onclick={() => media('Next')} title="Sonraki"><Icon name="next" size={20} /></button>
    </div>
    <div class="vol">
      <span class="muted small">Ses</span>
      <div class="seg">
        <button disabled={!on} onclick={() => vol('Down')} title="Sesi kıs"><Icon name="voldown" size={17} /></button>
        <button disabled={!on} onclick={() => vol('Mute')} title="Sessiz"><Icon name="mute" size={17} /></button>
        <button disabled={!on} onclick={() => vol('Up')} title="Sesi aç"><Icon name="volup" size={17} /></button>
      </div>
    </div>
  </section>

  <section class="card tight hint">
    <Icon name="phone" size={18} />
    <span class="muted small">
      Bilgisayarı telefondan yönetmek için telefondaki <b>Kumanda</b> sekmesini kullan: touchpad, klavye, medya, ses, sunum ve güç.
    </span>
  </section>
</div>

<style>
  .off {
    display: flex;
    gap: 12px;
    align-items: center;
    color: var(--muted);
  }
  .find {
    display: flex;
    align-items: center;
    gap: 26px;
    padding: 28px;
    overflow: hidden;
  }
  .find p {
    margin: 4px 0 14px;
  }
  .ringbtn {
    position: relative;
    z-index: 1;
  }
  .big {
    width: 88px;
    height: 88px;
    border-radius: 50%;
  }
  .big.stop {
    background: var(--violet);
    color: #fff;
    border-color: transparent;
    box-shadow: 0 10px 34px -8px rgba(139, 92, 246, 0.8);
  }
  .pulse {
    position: absolute;
    left: 28px;
    top: 50%;
    width: 88px;
    height: 88px;
    transform: translateY(-50%);
    pointer-events: none;
  }
  .pulse span {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    border: 2px solid var(--violet);
    opacity: 0;
  }
  .ringing .pulse span {
    animation: wave 2.1s ease-out infinite;
  }
  .ringing .pulse span:nth-child(2) {
    animation-delay: 0.7s;
  }
  .ringing .pulse span:nth-child(3) {
    animation-delay: 1.4s;
  }
  @keyframes wave {
    0% {
      transform: scale(1);
      opacity: 0.7;
    }
    100% {
      transform: scale(2.3);
      opacity: 0;
    }
  }
  .dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--violet);
    animation: blink 1s ease-in-out infinite;
  }
  @keyframes blink {
    50% {
      opacity: 0.2;
    }
  }
  .row h2 {
    margin-right: 4px;
  }
  .media h3 {
    margin-bottom: 16px;
  }
  .np {
    gap: 16px;
  }
  .art {
    width: 64px;
    height: 64px;
    border-radius: 18px;
    flex: none;
    display: grid;
    place-items: center;
    background: linear-gradient(135deg, rgba(139, 92, 246, 0.28), rgba(227, 169, 43, 0.2));
    border: 1px solid var(--line-2);
  }
  .art.playing {
    box-shadow: 0 0 0 1px var(--gold-line), 0 12px 32px -12px rgba(227, 169, 43, 0.6);
  }
  .tt {
    font-size: 16px;
    font-weight: 620;
  }
  .controls {
    display: flex;
    justify-content: center;
    align-items: center;
    gap: 22px;
    margin: 22px 0 18px;
  }
  .lg {
    width: 46px;
    height: 46px;
    border-radius: 50%;
  }
  .play {
    width: 60px;
    height: 60px;
    border-radius: 50%;
  }
  .vol {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 14px;
    padding-top: 16px;
    border-top: 1px solid var(--line);
  }
  .seg {
    display: flex;
    background: var(--s3);
    border: 1px solid var(--line);
    border-radius: 12px;
    padding: 3px;
    gap: 3px;
  }
  .seg button {
    width: 52px;
    height: 32px;
    border: none;
    background: transparent;
    border-radius: 9px;
  }
  .seg button:hover:not(:disabled) {
    background: var(--s4);
    border: none;
  }
  .hint {
    display: flex;
    gap: 12px;
    align-items: center;
    color: var(--muted);
  }
</style>
