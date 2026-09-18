import { createRouter, createWebHashHistory, type RouteRecordRaw } from 'vue-router'

const routes: RouteRecordRaw[] = [
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
        component: () => import('../views/ArtistView.vue'),
    },
    {
        path: '/album',
        name: 'album',
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
]

const router = createRouter({
    history: createWebHashHistory(),
    routes,
})

export default router