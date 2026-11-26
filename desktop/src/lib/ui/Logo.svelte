<script lang="ts">
  // T-Bridge işareti: T'nin üst çizgisi köprü kemeri (assets/logo.svg ile aynı geometri).
  let { size = 32 }: { size?: number } = $props()
  const id = `lg${Math.random().toString(36).slice(2, 8)}`
</script>

<svg width={size} height={size} viewBox="0 0 512 512" aria-label="T-Bridge">
  <defs>
    <linearGradient {id} gradientUnits="userSpaceOnUse" x1="110" y1="90" x2="400" y2="430">
      <stop offset="0" stop-color="#F7CF5A" />
      <stop offset="0.55" stop-color="#D99A0B" />
      <stop offset="1" stop-color="#9C6400" />
    </linearGradient>
  </defs>
  <rect width="512" height="512" rx="116" fill="#0d0c0a" />
  <rect x="5" y="5" width="502" height="502" rx="111" fill="none" stroke="url(#{id})" stroke-opacity="0.4" stroke-width="6" />
  <path d="M100 222 Q256 92 412 222" fill="none" stroke="url(#{id})" stroke-width="46" stroke-linecap="round" />
  <path d="M256 168 V404" stroke="url(#{id})" stroke-width="54" stroke-linecap="round" />
  <path d="M120 226 V266 M392 226 V266" stroke="url(#{id})" stroke-width="22" stroke-linecap="round" opacity="0.85" />
</svg>
