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
  const media = (action: MediaAction) => send({ Media: { action } })
  const vol = (action: VolumeAction) => send({ Volume: { action } })
</script>

<div class="page">
  {#if !on}
    <div class="card muted">Telefon bağlı değil. Bağlanınca bu sayfadan telefonu kontrol edebilirsin.</div>
  {/if}

  <section class="card big">
    <div class="grow">
      <h2>Telefonu bul</h2>
      <div class="muted">Sessiz moddayken bile tam seste çalar.</div>
    </div>
    <button class:primary={!ringing} class:stop={ringing} disabled={!on} onclick={ring}>
      <Icon name="ring" /> {ringing ? 'Durdur' : 'Çaldır'}
    </button>
  </section>

  <section class="card">
    <h2>Telefonda çalan</h2>
    <div class="np">
      {#if s.nowPlaying}
        <div class="grow">
          <div class="title ellipsis">{s.nowPlaying.title || 'Bilinmeyen'}</div>
          <div class="muted ellipsis">{s.nowPlaying.artist}{s.nowPlaying.app_name ? ` · ${s.nowPlaying.app_name}` : ''}</div>
        </div>
      {:else}
        <div class="muted grow">Şu an çalan bir şey yok.</div>
      {/if}
    </div>
    <div class="controls">
      <button disabled={!on} onclick={() => media('Previous')} title="Önceki"><Icon name="prev" /></button>
      <button class="primary round" disabled={!on} onclick={() => media('PlayPause')} title="Oynat/Duraklat">
        <Icon name={s.nowPlaying?.playing ? 'pause' : 'play'} size={22} />
      </button>
      <button disabled={!on} onclick={() => media('Next')} title="Sonraki"><Icon name="next" /></button>
      <div class="sep"></div>
      <button disabled={!on} onclick={() => vol('Down')} title="Sesi kıs"><Icon name="voldown" /></button>
      <button disabled={!on} onclick={() => vol('Up')} title="Sesi aç"><Icon name="volup" /></button>
      <button disabled={!on} onclick={() => vol('Mute')} title="Sessiz"><Icon name="mute" /></button>
    </div>
  </section>

  <section class="card muted small">
    Bilgisayarı telefondan kontrol etmek için telefondaki <b>Kumanda</b> sekmesini kullan: touchpad, klavye, medya, ses, sunum ve güç.
  </section>
</div>

<style>
  .page {
    display: grid;
    gap: 14px;
  }
  .big {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .big button {
    padding: 10px 18px;
    font-size: 15px;
  }
  button.stop {
    background: var(--bad);
    border-color: var(--bad);
    color: #fff;
  }
  .np {
    display: flex;
    margin: 10px 0 14px;
    min-height: 40px;
    align-items: center;
  }
  .title {
    font-weight: 600;
    font-size: 15px;
  }
  .controls {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .controls button {
    padding: 9px;
  }
  .round {
    border-radius: 50%;
    padding: 12px !important;
  }
  .sep {
    width: 1px;
    height: 28px;
    background: var(--line);
    margin: 0 8px;
  }
  .small {
    font-size: 13px;
  }
</style>
