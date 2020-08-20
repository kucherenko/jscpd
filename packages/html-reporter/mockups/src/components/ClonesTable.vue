<template>
  <div class="w-full xl:w-8/12 mb-12 xl:mb-0 px-4">
    <div class="relative flex flex-col min-w-0 break-words bg-white w-full mb-6 shadow-lg rounded">
      <div class="rounded-t mb-0 px-4 py-3 border-0">
        <div class="flex flex-wrap items-center">
          <div class="relative w-full px-4 max-w-full flex-grow flex-1">
            <h3 class="font-semibold text-base text-gray-800">
              Clones detected in {{currentFormat}}
            </h3>
          </div>
          <div class="relative w-full font-semibold px-4 max-w-full flex-grow flex-1 text-right">
            Total: {{ statistics.formats[currentFormat].total.clones }}
          </div>
        </div>
      </div>
      <div class="block w-full overflow-x-auto">
        <!-- Projects table -->
        <table class="items-center w-full bg-transparent border-collapse">
          <thead>
          <tr>
            <th
              class="px-6 bg-gray-100 text-gray-600 align-middle border border-solid border-gray-200 py-3 text-xs uppercase border-l-0 border-r-0 whitespace-no-wrap font-semibold text-left">
              Files
            </th>
            <th
              class="px-6 bg-gray-100 text-gray-600 align-middle border border-solid border-gray-200 py-3 text-xs uppercase border-l-0 border-r-0 whitespace-no-wrap font-semibold text-left">
              Position
            </th>
          </tr>
          </thead>
          <tbody>
          <tr v-for="(clone, index) in duplicates.filter(c => c.format===currentFormat)" :key="index">
            <th
              class="border-t-0 px-6 align-middle border-l-0 border-r-0 text-xs whitespace-no-wrap p-4 text-left">
              <div>
                {{clone.firstFile.name}}<br/>
                {{clone.secondFile.name}}
              </div>
              <Prism :language="clone.format" :code="clone.fragment" />
            </th>

            <td
              class="border-t-0 font-semibold px-6 align-middle border-l-0 border-r-0 text-xs whitespace-no-wrap p-4 text-left">
            </td>
          </tr>
          </tbody>
        </table>
      </div>
    </div>
  </div>
</template>

<script>
import {mapActions, mapState} from "vuex";
import Prism from 'vue-prism-component'

export default {
  data() {
    return {};
  },
  methods: mapActions([
    'setCurrentFormat'
  ]),
  components: {
    Prism
  },
  computed: mapState({
    statistics: state => state.statistics,
    currentFormat: state => state.currentFormat,
    duplicates: state => state.duplicates,
  }),
};
</script>
