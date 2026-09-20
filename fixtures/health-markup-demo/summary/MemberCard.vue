<script lang="ts" setup>
import { ref } from "vue";

const expanded = ref(false);

function toggleDetails() {
  expanded.value = !expanded.value;
}

defineProps({
  member: { type: Object, required: true },
});
</script>

<template>
  <section class="member member--summary" data-testid="member-summary">
    <header class="member__header">
      <h3 class="member__name">{{ member.name }}</h3>
      <span class="member__status" v-if="member.active">active</span>
    </header>
    <p class="member__bio">{{ member.bio }}</p>
    <ul class="member__skills" v-if="expanded">
      <li class="member__skill" v-for="skill in member.skills" :key="skill">
        {{ skill }}
      </li>
    </ul>
    <footer class="member__footer">
      <button class="member__toggle" type="button" @click="toggleDetails">
        {{ expanded ? "Hide skills" : "Show skills" }}
      </button>
    </footer>
  </section>
</template>