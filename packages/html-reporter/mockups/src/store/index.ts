import Vue from 'vue'
import Vuex from 'vuex'
import {init} from "@/store/init";

Vue.use(Vuex)

export default new Vuex.Store({
  state: {
    ...init
  },
  mutations: {
    init(state, initial) {
      state.statistics = initial.statistics;
      state.duplicates = initial.duplicates;
    },
    setDuplicates(state, duplicates) {
      state.duplicates = duplicates;
    },
    setStatistics(state, statistics) {
      state.statistics = statistics;
    },
  },
  actions: {
    initData({commit}) {
      fetch('jscpd-report.json')
        .then((response: Response) => {
          return response.json();
        })
        .then(data => {
          commit('init', data)
        });
    }
  },
  modules: {}
})
