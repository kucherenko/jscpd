<template>
  <div>
    <sidebar-component menu-item="format" :current-format="$route.params.format"></sidebar-component>
    <div class="relative md:ml-64 bg-gray-200">
      <navbar-component brand="format"></navbar-component>
      <!-- Header -->
      <div class="relative bg-blue-600 md:pt-32 pb-32 pt-12">
        <div class="px-4 md:px-10 mx-auto w-full">
          <div>
            <!-- Card stats -->
            <Cards :info="statistics.formats[$route.params.format].total"/>
          </div>
          <div class="w-full lg:w-6/12 xl:w-3/12 px-4">
            <ul class="flex">
              <li class="-mb-px mr-1">
                <a
                  class="inline-block rounded-t py-2 px-4 font-semibold"
                  v-bind:class="{
                    'text-blue-200': activeTab!=='clones',
                    'border-b-2 text-white': activeTab==='clones'
                  }"
                  @click="activeTab='clones'">Clones
                </a>
              </li>
              <li class="mr-1">
                <a
                  class="inline-block py-2 px-4 hover:text-blue-200 font-semibold"
                  v-bind:class="{
                    'text-blue-200': activeTab!=='files',
                    'border-b-2 text-white': activeTab==='files'
                  }"
                  @click="activeTab='files'">Files
                </a>
              </li>
            </ul>
          </div>
        </div>
      </div>
      <div class="px-4 md:px-10 mx-auto w-full -m-24">
        <div class="flex flex-wrap mt-4">
          <Clones v-if="activeTab==='clones'"
                  :list="duplicates.filter(clone => clone.format === $route.params.format)"/>
          <SourcesTable
            v-if="activeTab==='files'"
            :sources="statistics.formats[$route.params.format].sources"/>
        </div>
        <Footer/>
      </div>
    </div>
  </div>
</template>
<script>
import NavbarComponent from "../components/Navbar.vue";
import SidebarComponent from "../components/Sidebar.vue";
import {mapState} from "vuex";
import Cards from "@/components/Cards";
import Footer from "@/components/Footer";
import Clones from "@/components/Clones";
import SourcesTable from "@/components/SourcesTable";

export default {
  name: "dashboard-page",
  components: {
    Clones,
    Footer,
    NavbarComponent,
    SidebarComponent,
    Cards,
    SourcesTable
  },
  data() {
    return {
      date: new Date().getFullYear(),
      activeTab: 'clones'
    }
  },
  computed: mapState({
    duplicates: state => state.duplicates,
    statistics: state => state.statistics,
    currentFormat: state => state.currentFormat,
  }),
};
</script>
