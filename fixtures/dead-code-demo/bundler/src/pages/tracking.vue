<script setup>
import { ref } from 'vue';

const code = ref('');
</script>

<template>
  <label>Tracking code <input v-model="code" /></label>
</template>
