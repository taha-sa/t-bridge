import { mount } from 'svelte'
import './app.css'
import App from './App.svelte'
import Pill from './Pill.svelte'
import Consent from './Consent.svelte'

// "#pill": ekran paylaşılırken üstte açılan küçük gösterge penceresi.
// "#consent": telefon ekran paylaşımı istediğinde açılan temalı onay penceresi.
const Root = location.hash === '#pill' ? Pill : location.hash === '#consent' ? Consent : App
const app = mount(Root, {
  target: document.getElementById('app')!,
})

export default app
