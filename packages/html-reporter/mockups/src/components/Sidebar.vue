<template>
  <nav
    class="md:left-0 md:block md:fixed md:top-0 md:bottom-0 md:overflow-y-auto md:flex-row md:flex-no-wrap md:overflow-hidden shadow-xl bg-white flex flex-wrap items-center justify-between relative md:w-64 z-10 py-4 px-6"
  >
    <div
      class="md:flex-col md:items-stretch md:min-h-full md:flex-no-wrap px-0 flex flex-wrap items-center justify-between w-full mx-auto"
    >
      <!-- Toggler -->
      <button
        class="cursor-pointer text-black opacity-50 md:hidden px-3 py-1 text-xl leading-none bg-transparent rounded border border-solid border-transparent"
        type="button"
        v-on:click="toggleCollapseShow('bg-white m-2 py-3 px-6')"
      >
        <i class="fas fa-bars"></i>
      </button>
      <!-- Brand -->
      <router-link
        class="md:block text-left md:pb-2 text-gray-700 mr-0 inline-block whitespace-no-wrap text-sm uppercase font-bold p-4 px-0"
        to="/"
      >
        <img class="m-2 max-w-full h-auto align-middle border-none" src="../assets/logo-small-box.svg"/>
      </router-link>
      <!-- Collapse -->
      <div
        class="md:flex md:flex-col md:items-stretch md:opacity-100 md:relative md:mt-4 md:shadow-none shadow absolute top-0 left-0 right-0 z-40 overflow-y-auto overflow-x-hidden h-auto items-center flex-1 rounded"
        v-bind:class="collapseShow"
      >
        <!-- Collapse header -->
        <div
          class="md:min-w-full md:hidden block pb-4 mb-4 border-b border-solid border-gray-300"
        >
          <div class="flex flex-wrap">
            <div class="w-6/12">
              <router-link
                to="/"
                class="md:block text-left md:pb-2 text-gray-700 mr-0 inline-block whitespace-no-wrap text-sm uppercase font-bold p-4 px-0"

              >
                copy/paste report
              </router-link>
            </div>
            <div class="w-6/12 flex justify-end">
              <button
                type="button"
                class="cursor-pointer text-black opacity-50 md:hidden px-3 py-1 text-xl leading-none bg-transparent rounded border border-solid border-transparent"
                v-on:click="toggleCollapseShow('hidden')"
              >
                <i class="fas fa-times"></i>
              </button>
            </div>
          </div>
        </div>
        <!-- Navigation -->
        <ul class="md:flex-col md:min-w-full flex flex-col list-none">
          <li class="inline-flex">
            <router-link
              to="/"
              class="hover:text-grey-600 text-xs uppercase py-3 font-bold block"
              v-bind:class="{
                'text-gray-500': !menuItem,
                'text-gray-800': menuItem
              }"
            ><i class="fas fa-tv opacity-75 mr-2 text-sm"></i>
              Dashboard
            </router-link>
          </li>
          <li class="inline-flex">
            <router-link
              to="/clones"
              class="text-grey-500 hover:text-grey-600 text-xs uppercase py-3 font-bold block"
              v-bind:class="{
                'text-gray-500': menuItem === 'clones',
                'text-gray-800': menuItem !== 'clones'
              }"
            ><i class="fas fa-clone opacity-75 mr-2 text-sm"></i>
              All clones
            </router-link>
          </li>
        </ul>
        <hr class="my-4 md:min-w-full"/>
        <!-- Heading -->
        <FormatsList :list="formats" :current-format="currentFormat"/>
      </div>
    </div>
  </nav>
</template>
<script>
import NotificationDropdownComponent from "./NotificationDropdown.vue";
import UserDropdownComponent from "./UserDropdown.vue";
import FormatsList from "./FormatsList.vue";

export default {
  data() {
    return {
      collapseShow: "hidden"
    };
  },
  props: ['formats', 'currentFormat', 'menuItem'],
  methods: {
    toggleCollapseShow: function (classes) {
      this.collapseShow = classes;
    },
  },
  components: {
    FormatsList,
    NotificationDropdownComponent,
    UserDropdownComponent
  }
};
</script>
