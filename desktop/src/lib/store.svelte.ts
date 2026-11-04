import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { CoreEvent, Snapshot, TransferInfo } from './types'

const empty: Snapshot = {
  deviceName: '',
  fingerprint: '',
  port: 0,
  state: 'Disconnected',
  peer: null,
  peers: [],
  transfers: [],
  notifications: [],
  icons: {},
  battery: null,
  nowPlaying: null,
  settings: {
    clipboardSync: true,
    notifToast: true,
    transferToast: true,
    lowBatteryWarn: true,
    autostart: true,
    downloadDir: '',
    mutedApps: [],
  },
  downloadDir: '',
  error: null,
}

export const app = $state<{ s: Snapshot; ready: boolean; toast: string | null; lastClip: string | null }>({
  s: empty,
  ready: false,
  toast: null,
  lastClip: null,
})

const key = (t: { id: number; direction: string }) => `${t.direction}:${t.id}`
let toastTimer: ReturnType<typeof setTimeout> | undefined

export function flash(msg: string) {
  app.toast = msg
  clearTimeout(toastTimer)
  toastTimer = setTimeout(() => (app.toast = null), 3200)
}

export async function refresh() {
  app.s = await invoke<Snapshot>('snapshot')
  app.ready = true
}

function upsertTransfer(info: TransferInfo) {
  const list = app.s.transfers
  const i = list.findIndex((t) => key(t) === key(info))
  if (i >= 0) list[i] = info
  else list.push(info)
}

function onEvent(e: CoreEvent) {
  const s = app.s
  switch (e.type) {
    case 'state': {
      const ev = e as Extract<CoreEvent, { type: 'state' }>
      s.state = ev.state
      s.peer = ev.peer
      if (ev.state !== 'Connected') {
        s.battery = null
        s.nowPlaying = null
      }
      break
    }
    case 'paired': {
      const ev = e as Extract<CoreEvent, { type: 'paired' }>
      s.peers = [ev.peer, ...s.peers.filter((p) => p.id !== ev.peer.id)]
      flash(`${ev.peer.name} eşleşti`)
      break
    }
    case 'clipboardReceived':
      app.lastClip = (e as Extract<CoreEvent, { type: 'clipboardReceived' }>).text
      break
    case 'transfer':
      upsertTransfer((e as Extract<CoreEvent, { type: 'transfer' }>).info)
      break
    case 'transferRemoved': {
      const ev = e as Extract<CoreEvent, { type: 'transferRemoved' }>
      s.transfers = s.transfers.filter((t) => key(t) !== key(ev))
      break
    }
    case 'notification': {
      const d = (e as Extract<CoreEvent, { type: 'notification' }>).data
      if (s.settings.mutedApps.includes(d.package)) break
      s.notifications = [d, ...s.notifications.filter((n) => n.key !== d.key)].slice(0, 200)
      break
    }
    case 'notificationRemoved': {
      const k = (e as Extract<CoreEvent, { type: 'notificationRemoved' }>).key
      s.notifications = s.notifications.filter((n) => n.key !== k)
      break
    }
    case 'appIconUrl': {
      const ev = e as Extract<CoreEvent, { type: 'appIconUrl' }>
      s.icons[ev.package] = ev.url
      break
    }
    case 'battery': {
      const ev = e as Extract<CoreEvent, { type: 'battery' }>
      s.battery = { level: ev.level, charging: ev.charging }
      break
    }
    case 'nowPlaying':
      s.nowPlaying = (e as Extract<CoreEvent, { type: 'nowPlaying' }>).info
      break
  }
}

export async function init() {
  await listen<CoreEvent>('kopru', (ev) => onEvent(ev.payload))
  await refresh()
}

export async function call<T = void>(cmd: string, args?: Record<string, unknown>): Promise<T | undefined> {
  try {
    return await invoke<T>(cmd, args)
  } catch (err) {
    flash(String(err))
    return undefined
  }
}

export function fmtBytes(n: number): string {
  if (n < 1024) return `${n} B`
  const u = ['KB', 'MB', 'GB', 'TB']
  let v = n / 1024
  let i = 0
  while (v >= 1024 && i < u.length - 1) {
    v /= 1024
    i++
  }
  return `${v.toFixed(v >= 100 ? 0 : 1)} ${u[i]}`
}

export function fmtTime(ms: number): string {
  const d = new Date(ms)
  const now = new Date()
  const hm = d.toLocaleTimeString('tr-TR', { hour: '2-digit', minute: '2-digit' })
  return d.toDateString() === now.toDateString() ? hm : `${d.toLocaleDateString('tr-TR')} ${hm}`
}
