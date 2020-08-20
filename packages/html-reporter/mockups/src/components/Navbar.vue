<template>
  <!-- Navbar -->
  <nav
    class="absolute top-0 left-0 w-full z-10 bg-transparent md:flex-row md:flex-no-wrap md:justify-start flex items-center p-4"
  >
    <div
      class="w-full mx-autp items-center flex justify-between md:flex-no-wrap flex-wrap md:px-10 px-4"
    >
      <!-- Brand -->
      <router-link
        v-if="!brand"
        to="/"
        class="text-white text-sm uppercase hidden lg:inline-block font-semibold"
      >Dashboard
      </router-link>
      <router-link
        v-if="brand === 'clones'"
        to="/clones"
        class="text-white text-sm uppercase hidden lg:inline-block font-semibold"
      >All Clones
      </router-link>
      <router-link
        v-if="brand === 'format'"
        :to="'/format/' + $route.params.format"
        class="text-white text-sm uppercase hidden lg:inline-block font-semibold"
      >Format: {{ $route.params.format }}
      </router-link>
    </div>
  </nav>
  <!-- End Navbar -->
</template>
<script>

export default {
  props: ['brand']
};
</script>
