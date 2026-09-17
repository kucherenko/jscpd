<script setup lang="ts">
import ShipmentRow from './components/ShipmentRow.vue';
import { formatWeight } from './units';
import { convertToPounds } from './units';

const shipments = [
  { id: 'AX-1', grams: 1200 },
  { id: 'AX-2', grams: 860 },
];
const pendingLabel = 'awaiting pickup';
</script>

<template>
  <section class="dispatch">
    <shipment-row
      v-for="shipment in shipments"
      :key="shipment.id"
      :grams="shipment.grams"
    />
    <footer>Heaviest: {{ formatWeight(shipments[0].grams) }}</footer>
  </section>
</template>
