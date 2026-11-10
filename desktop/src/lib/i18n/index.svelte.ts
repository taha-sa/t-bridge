import { invoke } from '@tauri-apps/api/core'
import tr from './tr.json'
import en from './en.json'
import de from './de.json'

export type Lang = 'tr' | 'en' | 'de'
/** Ayardaki değer: "system" sistem dilini izler. */
export type LangPref = 'system' | Lang
export type Key = keyof typeof tr
type Params = Record<string, string | number>
/** `x_one` / `x_other` çiftlerinin ortak kökü. */
export type PluralKey = Key extends infer K ? (K extends `${infer B}_other` ? B : never) : never

const dicts: Record<Lang, Record<string, string>> = { tr, en, de }
const LOCALES: Record<Lang, string> = { tr: 'tr-TR', en: 'en-US', de: 'de-DE' }

function guess(): Lang {
  const n = (typeof navigator === 'undefined' ? 'en' : navigator.language).toLowerCase()
  return n.startsWith('tr') ? 'tr' : n.startsWith('de') ? 'de' : 'en'
}

/** Etkin dil (reaktif). */
export const i18n = $state<{ lang: Lang }>({ lang: guess() })

function fill(s: string, p?: Params): string {
  return p ? s.replace(/\{(\w+)\}/g, (m, k) => (k in p ? String(p[k]) : m)) : s
}

export function t(key: Key, p?: Params): string {
  return fill(dicts[i18n.lang][key] ?? dicts.en[key] ?? key, p)
}

/** Çoğul: `key_one` (n = 1) veya `key_other`. `{n}` otomatik doldurulur. */
export function tn(key: PluralKey, n: number, p?: Params): string {
  const k = `${key}_${n === 1 ? 'one' : 'other'}` as Key
  return t(k, { n: fmtNum(n), ...p })
}

export const locale = () => LOCALES[i18n.lang]

export function fmtNum(n: number, digits = 0): string {
  return n.toLocaleString(locale(), { minimumFractionDigits: digits, maximumFractionDigits: digits })
}

/** Arka uç ayardan çözülmüş dili verir (sistem dili dahil). Pencere açılışında ve ayar değişince çağrılır. */
export async function syncLang() {
  try {
    const l = await invoke<string>('ui_lang')
    if (l === 'tr' || l === 'en' || l === 'de') i18n.lang = l
  } catch {
    /* arka uç hazır değil: tarayıcı dili kalır */
  }
  document.documentElement.lang = i18n.lang
}
