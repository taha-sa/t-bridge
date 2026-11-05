<script lang="ts">
  import { SIGNATURE } from './signature'
  import Icon from '../Icon.svelte'
  import { call } from '../store.svelte'
  const open = () => call('open_url', { url: 'https://instagram.com/tdesk.studio' })
</script>

<footer class="sig">
  <button class="ghost ig" onclick={open}>
    <Icon name="instagram" size={16} />
    <span>@tdesk.studio</span>
  </button>
  <svg class="name" viewBox="0 0 {SIGNATURE.w} {SIGNATURE.h}" role="img" aria-label="Taha Şaprak">
    <path d={SIGNATURE.d} />
  </svg>
</footer>

<style>
  .sig {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 28px 0 6px;
  }
  .ig {
    height: 30px;
    font-size: 13px;
    color: var(--muted);
  }
  .ig:hover:not(:disabled) {
    color: var(--gold-hi);
    background: transparent;
  }
  .name {
    height: 17px;
    width: auto;
    fill: var(--faint);
    opacity: 0.9;
  }
</style>
