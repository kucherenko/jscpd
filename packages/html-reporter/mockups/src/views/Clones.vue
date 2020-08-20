<template>
  <div>
    <sidebar-component menu-item="clones"></sidebar-component>
    <div class="relative md:ml-64 bg-gray-200">
      <navbar-component brand="clones"></navbar-component>
      <!-- Header -->
      <div class="relative bg-blue-600 md:pt-32 pb-32 pt-12">
        <div class="px-4 md:px-10 mx-auto w-full">
          <div>
            <!-- Card stats -->
            <Cards :info="statistics.total"/>
          </div>
        </div>
      </div>
      <div class="px-4 md:px-10 mx-auto w-full -m-24">
        <div class="flex flex-wrap mt-4">
          <Clones :list="duplicates" />
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
import FormatsTable from "@/components/FormatsTable";
import Footer from "@/components/Footer";
import Clones from "@/components/Clones";

export default {
  name: "dashboard-page",
  components: {
    Clones,
    Footer,
    NavbarComponent,
    SidebarComponent,
    Cards,
    FormatsTable
  },
  data() {
    return {
      date: new Date().getFullYear()
    }
  },
  computed: mapState({
    duplicates: state => state.duplicates,
    statistics: state => state.statistics,
    currentFormat: state => state.currentFormat,
  }),
};
</script>
