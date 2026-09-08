<template>
  <article class="ticket" :class="{ closed: ticket.closedAt }">
    <header class="ticket-header">
      <h3 class="ticket-title">{{ ticket.title }}</h3>
      <span class="ticket-id">#{{ ticket.id }}</span>
    </header>
    <p class="ticket-body">{{ ticket.body }}</p>
    <ul class="ticket-tags">
      <li v-for="tag in ticket.tags" :key="tag" class="ticket-tag">{{ tag }}</li>
    </ul>
    <footer class="ticket-footer">
      <time :datetime="ticket.createdAt">{{ formatDate(ticket.createdAt) }}</time>
      <button type="button" @click="$emit('select', ticket.id)">Open</button>
    </footer>
  </article>
</template>
<script setup>
import { formatDate } from '../lib/dates';
defineProps({ ticket: { type: Object, required: true } });
defineEmits(['select']);
</script>
<style scoped>
.ticket { border: 1px solid #ddd; border-radius: 6px; padding: 12px; }
.ticket.closed { opacity: 0.6; }
</style>
