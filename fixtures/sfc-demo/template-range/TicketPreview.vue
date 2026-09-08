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
<script>
export default {
  name: 'TicketPreview',
  props: { ticket: { type: Object, required: true } },
  emits: ['select'],
  methods: {
    formatDate(value) {
      return new Intl.DateTimeFormat('en', { dateStyle: 'medium' }).format(new Date(value));
    },
  },
};
</script>
<style>
.ticket-title { font-size: 1.1rem; margin: 0; }
.ticket-tag { display: inline-block; margin-right: 4px; }
.ticket-footer { display: flex; justify-content: space-between; }
</style>
