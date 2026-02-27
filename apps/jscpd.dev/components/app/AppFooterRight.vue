<script setup lang="ts">
const links = [
  {
    icon: 'i-simple-icons-github',
    label: 'GitHub',
    to: 'https://github.com/kucherenko/jscpd',
    target: '_blank' as const
  },
  {
    icon: 'i-simple-icons-x',
    label: 'Twitter',
    to: 'https://twitter.com/a_kucherenko',
    target: '_blank' as const
  },
  {
    icon: 'i-simple-icons-opencollective',
    label: 'Sponsor',
    to: 'https://opencollective.com/jscpd',
    target: '_blank' as const
  }
]
</script>

<template>
  <div class="flex items-center gap-4">
    <UButton
      v-for="link in links"
      :key="link.label"
      :icon="link.icon"
      :to="link.to"
      :target="link.target"
      :aria-label="link.label"
      color="neutral"
      variant="ghost"
      size="sm"
      :label="link.label"
    />
    <UColorModeButton />
  </div>
</template>