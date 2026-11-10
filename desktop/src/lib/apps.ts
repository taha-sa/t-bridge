// Ortak T-Suite uygulama kataloğu: eksik uygulama davetleri için ad, simge, indirme sayfası ve vurgu rengi.
import dl from '../assets/apps/t-downloader.png'
import ram from '../assets/apps/t-ramcash.png'
import tree from '../assets/apps/t-desktree.png'
import viewer from '../assets/apps/t-desk-viewer.png'

export type AppId = 't-downloader' | 't-ramcash' | 't-desktree' | 't-desk-viewer'

export interface AppInfo {
  name: string
  icon: string
  /** Resmi indirme sayfası (GitHub releases). */
  url: string
  /** Uygulamanın kendi tema rengi. */
  accent: string
}

export const APPS: Record<AppId, AppInfo> = {
  't-downloader': { name: 'T-Downloader', icon: dl, url: 'https://github.com/taha-sa/t-downloader-releases/releases/latest', accent: '#f5c542' },
  't-ramcash': { name: 'T-RamCash', icon: ram, url: 'https://github.com/taha-sa/t-ramcash-releases/releases/latest', accent: '#c8921f' },
  't-desktree': { name: 'T-DeskTree', icon: tree, url: 'https://github.com/taha-sa/t-desktree-releases/releases/latest', accent: '#2dd4bf' },
  't-desk-viewer': { name: 'T-Desk Viewer', icon: viewer, url: 'https://github.com/taha-sa/tdesk-viewer-releases/releases/latest', accent: '#8b9cff' },
}

export const isAppId = (id: string): id is AppId => id in APPS
