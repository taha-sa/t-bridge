import { mount } from 'svelte'
import './app.css'
import App from './App.svelte'
import Pill from './Pill.svelte'

// "#pill": ekran paylaşılırken üstte açılan küçük gösterge penceresi.
const Root = location.hash === '#pill' ? Pill : App
const app = mount(Root, {
  target: document.getElementById('app')!,
})

export default app
