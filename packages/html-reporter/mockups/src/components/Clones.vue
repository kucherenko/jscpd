<template>
  <div class="w-full xl:w-8/12 mb-12 xl:mb-0 px-4">
    <div v-for="(clone, index) in list" :key="index" class="relative flex flex-col min-w-0 break-words bg-white w-full mb-6 shadow-lg rounded">
      <Clone :clone="clone"/>
    </div>
  </div>
</template>

<script>
import Clone from "@/components/Clone";

export default {
  components: {
    Clone
  },
  props: ['list']
};
</script>
