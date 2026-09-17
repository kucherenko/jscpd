<script setup lang="ts">
defineProps<{ grams: number }>();
</script>

<template>
  <article class="shipment-row">{{ grams }} g</article>
</template>
