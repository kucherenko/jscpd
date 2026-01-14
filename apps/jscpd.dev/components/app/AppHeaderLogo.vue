<script setup lang="ts">
const appConfig = useAppConfig()
</script>

<template>
  <NuxtLink to="/" class="flex items-center gap-3 group">
    <UColorModeImage
      v-if="appConfig.header?.logo?.dark || appConfig.header?.logo?.light"
      :light="appConfig.header?.logo?.light || appConfig.header?.logo?.dark"
      :dark="appConfig.header?.logo?.dark || appConfig.header?.logo?.light"
      :alt="appConfig.header?.logo?.alt || appConfig.header?.title"
      class="h-8 w-auto shrink-0 transition-transform group-hover:scale-105"
    />
    <span v-else class="text-xl font-bold text-primary">
      {{ appConfig.header?.title || 'jscpd' }}
    </span>
    <UBadge color="primary" variant="solid" size="xs">v4</UBadge>
  </NuxtLink>
</template>