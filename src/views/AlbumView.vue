<template>
    <div class="album-view">
        <n-button @click="goBack">{{ backLabel }}</n-button>
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
                    <p>
                        <ArtistNames :platform="platform" :artists="album.artists" :fallback="album.artist"
                            @click-artist="openRelatedArtist" />
                    </p>
                    <p>{{ album.songCount }} 首<span v-if="album.publishDate"> · {{ album.publishDate }}</span></p>
                </div>
            </div>
            <SearchResultList v-if="songs.length" :songs="songs" v-model:selectedIds="selectedIds" @download="downloadSingle"
                @click-artist="openRelatedArtist" @click-album="openSongAlbum" />
            <n-empty v-else description="该专辑暂无可用歌曲" />
            <BatchDownloadBar v-if="selectedIds.length" :selected-count="selectedIds.length" @batch-download="downloadSelected" />
        </template>
    </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import { NButton, NSpin, NAlert, NEmpty } from 'naive-ui'
import type { AlbumInfo, SongInfo } from '../types'
import { fetchAlbumSongs } from '../api/musicApi'
import { useDownloadActions } from '../composables/useDownloadActions'
import SearchResultList from '../components/search/SearchResultList.vue'
import BatchDownloadBar from '../components/search/BatchDownloadBar.vue'
import ArtistNames from '../components/search/ArtistNames.vue'
import { useMusicNavigation } from '../composables/useMusicNavigation'

const route = useRoute()
const { openRelatedArtist, openSongAlbum, goBack, backLabel } = useMusicNavigation()
const query = { ...route.query }
const platform = typeof query.platform === 'string' ? query.platform : ''
const album = ref<AlbumInfo | null>(null)
const songs = ref<SongInfo[]>([])
const selectedIds = ref<string[]>([])
const loading = ref(false)
const error = ref('')
const {
    downloadSingle,
    batchDownload
} = useDownloadActions()
async function loadAlbum() {
    if (loading.value) return
    album.value = null
    songs.value = []
    selectedIds.value = []
    error.value = ''
    loading.value = true
    try {
        if (typeof query.platform !== 'string' || typeof query.id !== 'string') throw new Error('缺少专辑信息，请返回搜索重新选择')
        const result = await fetchAlbumSongs(query.platform, query.id)
        album.value = {
            ...result.album,
            name: result.album.name || (typeof query.name === 'string' ? query.name : ''),
            artist: result.album.artist || (typeof query.artist === 'string' ? query.artist : ''),
            publishDate: result.album.publishDate || (typeof query.date === 'string' ? query.date : ''),
        }
        songs.value = result.songs
    } catch (e) {
        error.value = String(e)
    } finally {
        loading.value = false
    }
}

onMounted(loadAlbum)

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
