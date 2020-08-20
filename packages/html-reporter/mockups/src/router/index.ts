import Vue from 'vue'
import VueRouter, {RouteConfig} from 'vue-router'
import Dashboard from '../views/Dashboard.vue'
import Format from '../views/Format.vue'
import Clones from '../views/Clones.vue'

Vue.use(VueRouter)

const routes: Array<RouteConfig> = [
  {
    path: '/',
    name: 'Dashboard',
    component: Dashboard
  },
  {
    path: '/clones',
    name: 'Clones',
    component: Clones
  },
  {
    path: '/format/:format',
    name: 'Format',
    component: Format
  }
]

const router = new VueRouter({
  // mode: 'history',
  base: process.env.BASE_URL,
  routes
})

export default router
