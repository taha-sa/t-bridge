// Rust çekirdeğindeki serde tipleriyle birebir (bkz. core/src/types.rs).
export type ConnState = 'Disconnected' | 'Connecting' | 'Connected'
export type Direction = 'Outgoing' | 'Incoming'
export type TransferState = 'Queued' | 'Active' | 'Paused' | 'Done' | 'Failed' | 'Cancelled'
export type MediaAction = 'PlayPause' | 'Next' | 'Previous' | 'Stop'
export type VolumeAction = 'Up' | 'Down' | 'Mute'

export interface PeerInfo {
  id: string
  name: string
  platform: 'Desktop' | 'Android'
  fingerprint: string
  addrs: string[]
}

export interface TransferInfo {
  id: number
  direction: Direction
  name: string
  size: number
  done: number
  state: TransferState
  speed: number
  detail: string | null
}

export interface NotificationData {
  key: string
  package: string
  app_name: string
  title: string
  text: string
  time: number
}

export interface NowPlaying {
  title: string
  artist: string
  playing: boolean
  app_name: string
}

export interface Battery {
  level: number
  charging: boolean
}

export interface Settings {
  clipboardSync: boolean
  notifMirror: boolean
  notifSound: boolean
  notifToast: boolean
  settingsVersion: number
  transferToast: boolean
  lowBatteryWarn: boolean
  autostart: boolean
  downloadDir: string
  mutedApps: string[]
  theme: Theme
  autoUpdate: boolean
  screenShare: boolean
  screenAllowed: string[]
}

export type Theme = 'dark' | 'system' | 'light'

export type UpdatePhase = 'idle' | 'checking' | 'latest' | 'downloading' | 'ready' | 'installing' | 'error'
export interface UpdateState {
  phase: UpdatePhase
  version: string | null
  notes: string | null
  progress: number
  error: string | null
}

export interface Snapshot {
  deviceName: string
  fingerprint: string
  port: number
  state: ConnState
  peer: PeerInfo | null
  peers: PeerInfo[]
  transfers: TransferInfo[]
  notifications: NotificationData[]
  icons: Record<string, string>
  battery: Battery | null
  nowPlaying: NowPlaying | null
  settings: Settings
  downloadDir: string
  error: string | null
  firewallOk: boolean
  version: string
  update: UpdateState
}

export interface Pairing {
  code: string
  svg: string
  addrs: string[]
  name: string
}

export type PhoneCommand =
  | { Ring: { on: boolean } }
  | { Media: { action: MediaAction } }
  | { Volume: { action: VolumeAction } }
  | { DismissNotification: { key: string } }

export type CoreEvent =
  | { type: 'state'; state: ConnState; peer: PeerInfo | null }
  | { type: 'paired'; peer: PeerInfo }
  | { type: 'pairFailed'; reason: string }
  | { type: 'clipboardReceived'; text: string }
  | { type: 'transfer'; info: TransferInfo }
  | { type: 'transferRemoved'; id: number; direction: Direction }
  | { type: 'notification'; data: NotificationData }
  | { type: 'notificationRemoved'; key: string }
  | { type: 'notificationExt'; key: string; can_reply: boolean; choices: string[] }
  | { type: 'notificationReplyResult'; key: string; ok: boolean; error: string | null }
  | { type: 'appIconUrl'; package: string; url: string }
  | { type: 'battery'; level: number; charging: boolean }
  | { type: 'nowPlaying'; info: NowPlaying | null }
  | { type: string; [k: string]: unknown }
