<template>
  <div class="flex flex-wrap">
    <div class="w-full lg:w-6/12 xl:w-3/12 px-4">
      <div class="relative flex flex-col min-w-0 break-words bg-white rounded mb-6 xl:mb-0 shadow-lg">
        <div class="flex-auto p-4">
          <div class="flex flex-wrap">
            <div class="relative w-full pr-4 max-w-full flex-grow flex-1">
              <h5 class="text-gray-500 uppercase font-bold text-xs">
                Total files
              </h5>
              <span class="font-semibold text-xl text-gray-800">{{ info.sources }}</span>
            </div>
            <div class="relative w-auto pl-4 flex-initial">
              <div
                class="text-white p-3 text-center inline-flex items-center justify-center w-12 h-12 shadow-lg rounded-full bg-pink-700">
                <i class="far fa-chart-bar"></i>
              </div>
            </div>
          </div>
          <p class="text-sm text-gray-500 mt-4">
                      <span class="text-green-500 mr-2">
                        {{ info.lines }}
                      </span>
            <span class="whitespace-no-wrap">
                        Lines of of code in the files
                      </span>
          </p>
        </div>
      </div>
    </div>
    <div class="w-full lg:w-6/12 xl:w-3/12 px-4">
      <div class="relative flex flex-col min-w-0 break-words bg-white rounded mb-6 xl:mb-0 shadow-lg">
        <div class="flex-auto p-4">
          <div class="flex flex-wrap">
            <div class="relative w-full pr-4 max-w-full flex-grow flex-1">
              <h5 class="text-gray-500 uppercase font-bold text-xs">
                Clones
              </h5>
              <span class="font-semibold text-xl text-gray-800">
                          {{ info.clones }}
                        </span>
            </div>
            <div class="relative w-auto pl-4 flex-initial">
              <div
                class="text-white p-3 text-center inline-flex items-center justify-center w-12 h-12 shadow-lg rounded-full bg-pink-700">
                <i class="fas fa-chart-pie"></i>
              </div>
            </div>
          </div>
          <p class="text-sm text-gray-500 mt-4">
            <span class="text-red-500 mr-2">
             {{ info.duplicatedLines }} ({{ info.percentage }}%)
            </span>
            <span class="whitespace-no-wrap">Duplicated lines</span>
          </p>
        </div>
      </div>
    </div>
  </div>
</template>

<script>
export default {
  data() {
    return {};
  },
  props: ['info'],
};
</script>
