<script lang="ts">
  import { flip } from 'svelte/animate'
  import { fly } from 'svelte/transition'
  import { app, call, flash, fmtBytes } from './store.svelte'
  import { t, tn, fmtNum, type Key } from './i18n/index.svelte'
  import type { TransferInfo } from './types'
  import Icon from './Icon.svelte'
  import Progress from './ui/Progress.svelte'

  const s = $derived(app.s)
  const on = $derived(s.state === 'Connected')

  const running = $derived(s.transfers.filter((x) => x.state === 'Active'))
  const waiting = $derived(s.transfers.filter((x) => x.state === 'Queued' || x.state === 'Paused'))
  const finished = $derived(
    s.transfers.filter((x) => ['Done', 'Failed', 'Cancelled'].includes(x.state)).sort((a, b) => b.id - a.id),
  )
  const groups = $derived(
    [
      { key: 'run', title: t('files.groupRun'), items: running },
      { key: 'wait', title: t('files.groupWait'), items: waiting },
      { key: 'done', title: t('files.groupDone'), items: finished },
    ].filter((g) => g.items.length),
  )
  const totalSpeed = $derived(running.reduce((a, x) => a + x.speed, 0))

  const stateLabel = (st: string) => t(`files.state.${st}` as Key)

  const pct = (x: TransferInfo) => (x.size === 0 ? (x.state === 'Done' ? 100 : 0) : Math.min(100, (x.done / x.size) * 100))

  function eta(x: TransferInfo) {
    if (x.state !== 'Active' || x.speed <= 0) return ''
    const sec = Math.round((x.size - x.done) / x.speed)
    if (sec < 60) return t('files.etaSec', { n: sec })
    if (sec < 3600) return t('files.etaMin', { n: Math.round(sec / 60) })
    return t('files.etaHour', { n: fmtNum(sec / 3600, 1) })
  }

  function kind(name: string): string {
    const ext = name.split('.').pop()?.toLowerCase() ?? ''
    if (['jpg', 'jpeg', 'png', 'gif', 'webp', 'heic', 'bmp', 'svg'].includes(ext)) return 'image'
    if (['mp4', 'mov', 'mkv', 'avi', 'webm', '3gp'].includes(ext)) return 'video'
    if (['mp3', 'm4a', 'wav', 'flac', 'ogg', 'aac', 'opus'].includes(ext)) return 'music'
    if (['zip', 'rar', '7z', 'tar', 'gz', 'apk'].includes(ext)) return 'archive'
    return 'doc'
  }

  async function pick(folder: boolean) {
    const n = await call<number>('pick_and_send', { folder })
    if (n) flash(tn('files.queued', n))
  }

  const args = (x: TransferInfo) => ({ id: x.id, direction: x.direction })
  const shortName = (n: string) => n.split('/').pop() ?? n
  const dirName = (n: string) => (n.includes('/') ? n.slice(0, n.lastIndexOf('/')) : '')
  const tone = (x: TransferInfo) => (x.state === 'Done' ? 'ok' : x.state === 'Failed' ? 'bad' : x.state === 'Active' ? 'gold' : 'muted')
</script>

<div class="page">
  <section class="zone card" class:disabled={!s.peers.length}>
    <div class="zi"><Icon name="upload" size={26} /></div>
    <div class="grow">
      <h2>{t('files.dropTitle')}</h2>
      <p class="muted">
        {#if !s.peers.length}
          {t('files.needPhone')}
        {:else if on}
          {t('files.ready', { name: s.peer?.name ?? '' })}
        {:else}
          {t('files.offline')}
        {/if}
      </p>
    </div>
    <div class="zb">
      <button class="primary" disabled={!s.peers.length} onclick={() => pick(false)}><Icon name="doc" size={16} /> {t('files.pickFile')}</button>
      <button disabled={!s.peers.length} onclick={() => pick(true)}><Icon name="folder" size={16} /> {t('files.pickFolder')}</button>
    </div>
  </section>

  <div class="row bar">
    {#if running.length}
      <span class="chip gold num"><span class="live"></span>{fmtBytes(totalSpeed)}{t('unit.perSec')}</span>
    {/if}
    <div class="grow"></div>
    <button class="ghost sm" onclick={() => call('open_downloads')}><Icon name="folder" size={15} /> {t('files.openInbox')}</button>
    {#if finished.length}
      <button class="ghost sm" onclick={() => call('clear_finished')}><Icon name="trash" size={15} /> {t('files.clearDone')}</button>
    {/if}
  </div>

  {#if groups.length === 0}
    <section class="empty card">
      <div class="art">
        <div class="sheet a"></div>
        <div class="sheet b"></div>
        <div class="sheet c"><Icon name="files" size={22} /></div>
      </div>
      <h2>{t('files.emptyTitle')}</h2>
      <p class="muted">
        {t('files.emptyBody')}<br /><span class="selectable path">{s.downloadDir}</span>
      </p>
    </section>
  {:else}
    {#each groups as g (g.key)}
      <div class="group">
        <h3>{g.title} <span class="faint num">{g.items.length}</span></h3>
        <div class="list">
          {#each g.items as x (x.direction + x.id)}
            <div class="item card tight" class:active={x.state === 'Active'} animate:flip={{ duration: 260 }} in:fly={{ y: 8, duration: 240 }}>
              <div class="fi {kind(x.name)}">
                <Icon name={kind(x.name)} size={19} />
                <span class="arrow" class:in={x.direction === 'Incoming'}><Icon name={x.direction === 'Incoming' ? 'down' : 'up'} size={11} /></span>
              </div>
              <div class="grow body">
                <div class="row top">
                  <div class="grow ellipsis" title={x.name}>
                    <span class="fn">{shortName(x.name)}</span>
                    {#if dirName(x.name)}<span class="faint small"> · {dirName(x.name)}</span>{/if}
                  </div>
                  <div class="small muted num sz">{fmtBytes(x.done)} / {fmtBytes(x.size)}</div>
                </div>
                <Progress value={pct(x)} active={x.state === 'Active'} tone={tone(x)} />
                <div class="row meta small">
                  <span class="st {x.state}">{stateLabel(x.state)}</span>
                  <span class="muted grow ellipsis">
                    {#if x.state === 'Active'}{fmtBytes(x.speed)}{t('unit.perSec')} · {eta(x)}{/if}
                    {#if x.state === 'Failed' && x.detail}{x.detail}{/if}
                    {#if x.state === 'Queued' && !on}{t('files.waitingConn')}{/if}
                    {#if x.state !== 'Failed'}{x.direction === 'Incoming' ? t('files.fromPhone') : t('files.toPhone')}{/if}
                  </span>
                  <div class="acts">
                    {#if x.state === 'Active' || x.state === 'Queued'}
                      <button class="ghost icon sm" title={t('files.pause')} onclick={() => call('pause_transfer', args(x))}><Icon name="pause" size={15} /></button>
                    {/if}
                    {#if x.state === 'Paused'}
                      <button class="ghost icon sm" title={t('files.resume')} onclick={() => call('resume_transfer', args(x))}><Icon name="play" size={15} /></button>
                    {/if}
                    {#if ['Active', 'Queued', 'Paused'].includes(x.state)}
                      <button class="ghost icon sm" title={t('files.cancel')} onclick={() => call('cancel_transfer', args(x))}><Icon name="x" size={15} /></button>
                    {/if}
                    {#if x.state === 'Done' && x.direction === 'Incoming' && x.detail}
                      <button class="ghost sm" onclick={() => call('reveal', { path: x.detail })}><Icon name="folder" size={14} /> {t('files.show')}</button>
                    {/if}
                  </div>
                </div>
              </div>
            </div>
          {/each}
        </div>
      </div>
    {/each}
  {/if}
</div>

<style>
  .zone {
    display: flex;
    align-items: center;
    gap: 18px;
    padding: 22px 24px;
    border-style: dashed;
    border-color: rgba(227, 169, 43, 0.25);
    background:
      radial-gradient(500px 160px at 0% 0%, rgba(227, 169, 43, 0.08), transparent 70%),
      linear-gradient(180deg, var(--s2), var(--s1));
  }
  .zone {
    flex-wrap: wrap;
  }
  .zone .grow {
    min-width: 260px;
  }
  .zb {
    display: flex;
    gap: 10px;
  }
  .zone.disabled {
    border-color: var(--line-2);
  }
  .zone p {
    margin: 3px 0 0;
  }
  .zi {
    width: 54px;
    height: 54px;
    border-radius: 16px;
    flex: none;
    display: grid;
    place-items: center;
    color: var(--gold-hi);
    background: var(--gold-soft);
    border: 1px solid var(--gold-line);
  }
  .bar {
    min-height: 30px;
  }
  .live {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--gold-hi);
    animation: blink 1.2s ease-in-out infinite;
  }
  @keyframes blink {
    50% {
      opacity: 0.25;
    }
  }
  .group h3 {
    margin: 6px 2px 10px;
  }
  .list {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .item {
    display: flex;
    gap: 14px;
    align-items: center;
    border-radius: var(--r);
  }
  .item.active {
    border-color: rgba(227, 169, 43, 0.22);
  }
  .fi {
    position: relative;
    width: 44px;
    height: 44px;
    border-radius: 13px;
    flex: none;
    display: grid;
    place-items: center;
    background: var(--s3);
    border: 1px solid var(--line);
    color: var(--text-2);
  }
  .fi.image {
    color: #f5c542;
  }
  .fi.video {
    color: #b9a1ff;
  }
  .fi.music {
    color: #f0a3c8;
  }
  .fi.archive {
    color: #9ad1a8;
  }
  .arrow {
    position: absolute;
    right: -4px;
    bottom: -4px;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--gold-grad);
    color: var(--on-gold);
    border: 2px solid var(--s1);
  }
  .arrow.in {
    background: var(--s4);
    color: var(--text);
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .fn {
    font-weight: 600;
  }
  .sz {
    flex: none;
  }
  .meta {
    gap: 10px;
    min-height: 30px;
    margin: -4px 0 -6px;
  }
  .st {
    font-weight: 600;
    color: var(--text-2);
  }
  .st.Active {
    color: var(--gold-hi);
  }
  .st.Done {
    color: var(--ok);
  }
  .st.Failed {
    color: var(--bad);
  }
  .acts {
    display: flex;
    gap: 2px;
  }
  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    padding: 46px 20px;
  }
  .empty p {
    margin: 6px 0 0;
  }
  .path {
    color: var(--text-2);
    font-size: 12px;
  }
  .art {
    position: relative;
    width: 92px;
    height: 76px;
    margin-bottom: 18px;
  }
  .sheet {
    position: absolute;
    width: 54px;
    height: 66px;
    border-radius: 12px;
    border: 1px solid var(--line-2);
    background: var(--s3);
  }
  .sheet.a {
    left: 4px;
    top: 8px;
    transform: rotate(-12deg);
    opacity: 0.6;
  }
  .sheet.b {
    right: 4px;
    top: 8px;
    transform: rotate(12deg);
    opacity: 0.6;
  }
  .sheet.c {
    left: 19px;
    top: 2px;
    display: grid;
    place-items: center;
    color: var(--gold-hi);
    border-color: var(--gold-line);
    background: linear-gradient(180deg, var(--s4), var(--s2));
  }
</style>
