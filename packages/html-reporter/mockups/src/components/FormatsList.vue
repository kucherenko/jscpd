<template>
  <div>
    <h6 class="md:min-w-full text-gray-600 text-xs uppercase font-bold block pt-1 pb-4 no-underline">Formats</h6>
    <router-link
      :to="'/format/' + format"
      v-for="(format, index) in Object.keys(statistics.formats)"
      v-bind:key="index"
      class="text-white active:bg-blue-600 font-bold uppercase text-xs px-2 py-1 rounded shadow hover:shadow-md outline-none focus:outline-none mr-1 mb-1"
      type="button"
      v-bind:class="{
        'bg-blue-500': format !== currentFormat,
        'bg-gray-500': format === currentFormat,
      }"
      style="transition: all .15s ease">
      {{ format }}
    </router-link>
  </div>
</template>

<script>

import {mapState} from "vuex";

export default {
  data() {
    return {};
  },
  props: ['currentFormat'],

  computed: mapState({
    statistics: state => state.statistics,
  }),
};
</script>
