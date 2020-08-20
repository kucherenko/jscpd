import Vue from 'vue'
import App from './App.vue'
import router from './router'
import store from './store'

//Styles
import '@fortawesome/fontawesome-free/css/all.min.css';
import './assets/tailwind.css'

Vue.config.productionTip = false

new Vue({
  router,
  store,
  render: h => h(App),
  created() {
    this.$store.dispatch('initData');
  },
}).$mount('#app')
