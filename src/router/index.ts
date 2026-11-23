import {
    createRouter,
    createWebHashHistory,
    type RouteRecordRaw,
} from 'vue-router'
import { isNativeRuntime } from '../api/runtimeApi'

const routes: RouteRecordRaw[] = [
    {
        path: '/playlist/monitors',
        meta: { keepAlive: true },
        component: () => import('../views/MonitorView.vue'),
        beforeEnter: () => (isNativeRuntime() ? '/playlist' : true),
    },
    {
        path: '/',
        redirect: '/search',
    },
    {
        path: '/search',
        name: 'search',
        component: () => import('../views/SearchView.vue'),
        meta: { keepAlive: true },
    },
    {
        path: '/artist',
        name: 'artist',
        meta: { keepAlive: true },
        component: () => import('../views/ArtistView.vue'),
    },
    {
        path: '/album',
        name: 'album',
        meta: { keepAlive: true },
        component: () => import('../views/AlbumView.vue'),
    },
    {
        path: '/playlist',
        name: 'playlist',
        component: () => import('../views/PlaylistView.vue'),
        meta: { keepAlive: true },
    },
    {
        path: '/task',
        name: 'task',
        component: () => import('../views/TaskView.vue'),
        meta: { keepAlive: true },
    },
    {
        path: '/settings',
        name: 'settings',
        component: () => import('../views/SettingsView.vue'),
        meta: { keepAlive: true },
    },
    {
        path: '/settings/about',
        name: 'about',
        component: () => import('../views/AboutView.vue'),
        meta: { keepAlive: false },
    },
    {
        path: '/player',
        name: 'player',
        component: () => import('../views/PlayerView.vue'),
        meta: { keepAlive: false, fullscreen: true },
    },
]

const router = createRouter({
    history: createWebHashHistory(),
    routes,
})

export default router
