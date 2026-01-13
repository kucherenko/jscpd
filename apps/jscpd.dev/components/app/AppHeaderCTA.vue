<script setup lang="ts">
const links = [
  {
    icon: 'i-simple-icons-github',
    to: 'https://github.com/kucherenko/jscpd',
    target: '_blank' as const,
    ariaLabel: 'GitHub'
  },
  {
    icon: 'i-simple-icons-npm',
    to: 'https://www.npmjs.com/package/jscpd',
    target: '_blank' as const,
    ariaLabel: 'npm'
  },
  {
    icon: 'i-simple-icons-opencollective',
    to: 'https://opencollective.com/jscpd',
    target: '_blank' as const,
    ariaLabel: 'OpenCollective'
  }
]
</script>

<template>
  <div class="flex items-center gap-1">
    <UButton
      v-for="link in links"
      :key="link.ariaLabel"
      :icon="link.icon"
      :to="link.to"
      :target="link.target"
      :aria-label="link.ariaLabel"
      color="neutral"
      variant="ghost"
      size="sm"
    />
  </div>
</template>