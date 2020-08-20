<template>
  <div>
    <div class="md:flex">
      <div class="mt-4 md:mt-0 md:ml-6 p-5">
        <div class="uppercase tracking-wide text-left text-sm text-indigo-600 font-bold">{{ clone.format }}</div>
        <p class="mt-2 text-gray-800 text-left">
          {{ clone.firstFile.name }}
          <span
            class="text-gray-600">({{ clone.firstFile.startLoc.line }}:{{
              clone.firstFile.startLoc.column
            }}-{{ clone.firstFile.endLoc.line }}:{{ clone.firstFile.endLoc.column }})</span>
        </p>
        <p class="mt-2 text-gray-800 text-left">
          {{ clone.secondFile.name }}
          <span
            class="text-gray-600">({{ clone.secondFile.startLoc.line }}:{{
              clone.secondFile.startLoc.column
            }}-{{ clone.secondFile.endLoc.line }}:{{ clone.secondFile.endLoc.column }})</span>
        </p>
      </div>
    </div>
    <div class="text-center">
      <button @click="toggleCode()" v-if="!isActive" class="text-grey-200 text-sm">
        Show code
      </button>
      <button @click="toggleCode()" v-if="isActive" class="text-grey-200 text-sm">
        Hide code
      </button>
    </div>
    <div v-if="isActive">
      <pre class="text-left bg-gray-100 p-5 text-sm">{{ clone.fragment }}</pre>
    </div>
  </div>
</template>

<script>

export default {
  data() {
    return {
      isActive: false,

    }
  },
  methods: {
    toggleCode() {
      this.isActive = !this.isActive;
    }
  },
  components: {
  },
  props: ['clone']
};
</script>
