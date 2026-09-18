<template>
    <div class="album-view">
        <n-button @click="router.push(backTarget)">{{ backTarget === '/search' ? '返回搜索' : '返回歌手' }}</n-button>
        <div v-if="loading" class="loading">
            <n-spin size="medium" description="正在获取专辑全部歌曲…" />
        </div>
        <n-alert v-else-if="error" type="error" title="获取专辑失败">
           {{ error }}
                    <n-button @click="loadAlbum">重试</n-button>
        </n-alert>
        <template v-else-if="album">
            <div class="album-header">
                <img v-if="album.coverUrl" :src="album.coverUrl" alt="专辑封面" />
                <div class="album-info">
                    <h2>{{ album.name || '专辑' }}</h2>
                    <p>{{ album.artist }}</p>
                    <p>{{ album.songCount }} 首<span v-if="album.publishDate"> · {{ album.publishDate }}</span></p>
                </div>
            </div>
            <SearchResultList v-if="songs.length" :songs="songs" v-model:selectedIds="selectedIds" @download="downloadSingle" />
            <n-empty v-else description="该专辑暂无可用歌曲" />
            <BatchDownloadBar v-if="selectedIds.length" :selected-count="selectedIds.length" @batch-download="downloadSelected" />
        </template>
    </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { NButton, NSpin, NAlert, NEmpty } from 'naive-ui'
import type { AlbumInfo, SongInfo } from '../types'
import { fetchAlbumSongs } from '../api/musicApi'
import { useDownloadActions } from '../composables/useDownloadActions'
import SearchResultList from '../components/search/SearchResultList.vue'
import BatchDownloadBar from '../components/search/BatchDownloadBar.vue'

const route = useRoute()
const router = useRouter()
const backTarget = computed(() => {
    const target = route.query.returnTo
    return typeof target === 'string' && target.startsWith('/artist?') ? target : '/search'
})
const album = ref<AlbumInfo | null>(null)
const songs = ref<SongInfo[]>([])
const selectedIds = ref<string[]>([])
const loading = ref(false)
const error = ref('')
const {
    downloadSingle,
    batchDownload
} = useDownloadActions()
let generation = 0

async function loadAlbum() {
    if (route.path !== '/album') return
    const request = ++generation
    const query = { ...route.query }
    album.value = null
    songs.value = []
    selectedIds.value = []
    error.value = ''
    loading.value = true
    try {
        if (typeof query.platform !== 'string' || typeof query.id !== 'string') throw new Error('缺少专辑信息，请返回搜索重新选择')
        const result = await fetchAlbumSongs(query.platform, query.id)
        if (request !== generation) return
        album.value = {
            ...result.album,
            name: typeof query.name === 'string' ? query.name : result.album.name,
            artist: typeof query.artist === 'string' ? query.artist : result.album.artist,
            publishDate: result.album.publishDate || (typeof query.date === 'string' ? query.date : ''),
        }
        songs.value = result.songs
    } catch (e) {
        if (request === generation) error.value = String(e)
    } finally {
        if (request === generation) loading.value = false
    }
}

watch(() => [route.path, route.query.platform, route.query.id], () => {
    ++generation
    if (route.path === '/album') void loadAlbum()
}, { immediate: true })

function downloadSelected() {
    batchDownload(songs.value.filter(song => selectedIds.value.includes(song.mid)))
}
</script>

<style scoped>
.album-view {
    display: flex;
    flex-direction: column;
    gap: 20px;
    min-width: 0;
}

.album-view > .n-button {
    align-self: flex-start;
}

.loading {
    display: flex;
    justify-content: center;
    padding: 48px 0;
}

.album-header {
    display: flex;
    align-items: center;
    gap: 20px;
}

.album-header img {
    width: 120px;
    height: 120px;
    flex-shrink: 0;
    object-fit: cover;
    border-radius: 10px;
}

.album-info {
    min-width: 0;
    overflow-wrap: anywhere;
}

.album-info h2 {
    margin: 0 0 8px;
}

.album-info p {
    margin: 4px 0;
    color: var(--color-text-secondary);
}

@media (max-width: 480px) {
    .album-header {
        gap: 12px;
    }

    .album-header img {
        width: 80px;
        height: 80px;
    }
}
</style>
