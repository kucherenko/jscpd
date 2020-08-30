<template>
  <div class="w-full xl:w-8/12 mb-12 xl:mb-0 px-4">
    <div class="relative flex flex-col min-w-0 break-words bg-white w-full mb-6 shadow-lg rounded">
      <div class="rounded-t mb-0 px-4 py-3 border-0">
        <div class="flex flex-wrap items-center">
          <div class="relative w-full px-4 max-w-full flex-grow flex-1">
            <h3 class="font-semibold text-base text-gray-800">
              Formats
            </h3>
          </div>
          <div class="relative w-full font-semibold px-4 max-w-full flex-grow flex-1 text-right">
            Total: {{ Object.keys(statistics.formats).length }}
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
            </th>
            <th
              class="px-6 bg-gray-100 text-gray-600 align-middle border border-solid border-gray-200 py-3 text-xs uppercase border-l-0 border-r-0 whitespace-no-wrap font-semibold text-left">
              Files
            </th>
            <th
              class="px-6 bg-gray-100 text-gray-600 align-middle border border-solid border-gray-200 py-3 text-xs uppercase border-l-0 border-r-0 whitespace-no-wrap font-semibold text-left">
              Lines
            </th>
            <th
              class="px-6 bg-gray-100 text-gray-600 align-middle border border-solid border-gray-200 py-3 text-xs uppercase border-l-0 border-r-0 whitespace-no-wrap font-semibold text-left">
              Clones
            </th>
            <th
              class="px-6 bg-gray-100 text-gray-600 align-middle border border-solid border-gray-200 py-3 text-xs uppercase border-l-0 border-r-0 whitespace-no-wrap font-semibold text-left">
              Duplicated lines
            </th>
            <th
              class="px-6 bg-gray-100 text-gray-600 align-middle border border-solid border-gray-200 py-3 text-xs uppercase border-l-0 border-r-0 whitespace-no-wrap font-semibold text-left">
              Duplicated tokens
            </th>
          </tr>
          </thead>
          <tbody>
          <tr v-for="(format, index) in Object.keys(statistics.formats)" :key="index">
            <th
              class="border-t-0 px-6 align-middle border-l-0 border-r-0 text-xs whitespace-no-wrap p-4 text-left">
              <router-link :to="'/format/' + format" class="font-semibold">{{ format }}</router-link>
            </th>
            <td class="border-t-0 px-6 align-middle border-l-0 border-r-0 text-xs whitespace-no-wrap p-4 text-left">
              {{ statistics.formats[format].total.sources }}
            </td>
            <td class="border-t-0 px-6 align-middle border-l-0 border-r-0 text-xs whitespace-no-wrap p-4 text-left">
              {{ statistics.formats[format].total.lines }}
            </td>
            <td class="border-t-0 px-6 align-middle border-l-0 border-r-0 text-xs whitespace-no-wrap p-4 text-left">
              {{ statistics.formats[format].total.clones }}
            </td>
            <td
              class="border-t-0 font-semibold px-6 align-middle border-l-0 border-r-0 text-xs whitespace-no-wrap p-4 text-left">
              <!--              <i class="fas fa-arrow-up text-green-500 mr-4"></i>-->
              {{ statistics.formats[format].total.duplicatedLines }}
              (<b>{{ statistics.formats[format].total.percentage }}%</b>)
            </td>
            <td
              class="border-t-0 font-semibold px-6 align-middle border-l-0 border-r-0 text-xs whitespace-no-wrap p-4 text-left">
              <!--              <i class="fas fa-arrow-up text-green-500 mr-4"></i>-->
              {{ statistics.formats[format].total.duplicatedTokens }}
              (<b>{{ statistics.formats[format].total.percentageTokens }}%</b>)
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

export default {
  data() {
    return {};
  },
  methods: mapActions([
    'setCurrentFormat'
  ]),
  computed: mapState({
    statistics: state => state.statistics
  }),
};
</script>
