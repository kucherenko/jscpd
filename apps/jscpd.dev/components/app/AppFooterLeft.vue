<script setup lang="ts">
const appConfig = useAppConfig()
</script>

<template>
  <div class="flex flex-col gap-4">
    <div class="flex items-center gap-2">
      <UColorModeImage
        v-if="appConfig.header?.logo?.dark || appConfig.header?.logo?.light"
        :light="appConfig.header?.logo?.light || appConfig.header?.logo?.dark"
        :dark="appConfig.header?.logo?.dark || appConfig.header?.logo?.light"
        :alt="appConfig.header?.logo?.alt || appConfig.header?.title"
        class="h-6 w-auto"
      />
      <span v-else class="text-lg font-bold">{{ appConfig.header?.title || 'jscpd' }}</span>
    </div>
    <div class="text-sm text-muted">
      {{ appConfig.footer?.credits || `Copyright © ${new Date().getFullYear()} jscpd` }}
    </div>
  </div>
</template>