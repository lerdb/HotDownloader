<template>
    <div class="artist-view">
        <n-button @click="router.push('/search')">返回搜索</n-button>
        <n-alert v-if="routeError" type="error" title="无法打开歌手">{{ routeError }}</n-alert>
        <template v-else-if="artist">
            <div class="artist-header">
                <img v-if="artist.coverUrl" :src="artist.coverUrl" alt="歌手头像" />
                <div class="artist-info">
                    <h2>{{ artist.name || '歌手' }}</h2>
                    <p v-if="artist.alias || artist.region">{{ [artist.alias, artist.region].filter(Boolean).join(' · ') }}</p>
                    <p>{{ songPage.total.value ?? artist.songCount }} 首歌曲 · {{ albumPage.total.value ?? artist.albumCount }} 张专辑</p>
                </div>
            </div>
            <div class="artist-tabs">
                <n-button :type="tab === 'songs' ? 'primary' : 'default'" @click="tab = 'songs'">歌曲</n-button>
                <n-button :type="tab === 'albums' ? 'primary' : 'default'" @click="showAlbums">专辑</n-button>
            </div>
            <template v-if="tab === 'songs'">
                <n-alert v-if="songPage.error.value" type="error" title="获取歌手歌曲失败">
                   {{ songPage.error.value }}
                    <n-button @click="songPage.loadMore">重试</n-button>
                </n-alert>
                <div v-if="songPage.loading.value && !songPage.loaded.value" class="loading">
                    <n-spin />
                </div>
                <SearchResultList v-if="songPage.items.value.length" :songs="songPage.items.value"
                    v-model:selectedIds="selectedIds" @download="downloadSingle" />
                <n-empty v-else-if="songPage.loaded.value && !songPage.error.value" description="暂无可用歌曲" />
                <LoadMoreButton v-if="songPage.hasMore.value" :loading="songPage.loading.value"
                    :disabled="songPage.loading.value" @click="songPage.loadMore" />
                <BatchDownloadBar v-if="selectedIds.length" :selected-count="selectedIds.length" @batch-download="downloadSelected" />
            </template>
            <template v-else>
                <n-alert v-if="albumPage.error.value" type="error" title="获取歌手专辑失败">
                   {{ albumPage.error.value }}
                    <n-button @click="albumPage.loadMore">重试</n-button>
                </n-alert>
                <div v-if="albumPage.loading.value && !albumPage.loaded.value" class="loading">
                    <n-spin />
                </div>
                <AlbumSearchResult v-if="albumPage.loaded.value" :albums="albumPage.items.value"
                    :has-more="false" :loading-more="false" @click-album="openAlbum" />
                <LoadMoreButton v-if="albumPage.hasMore.value" :loading="albumPage.loading.value"
                    :disabled="albumPage.loading.value" @click="albumPage.loadMore" />
            </template>
        </template>
    </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { NAlert, NButton, NEmpty, NSpin } from 'naive-ui'
import type { ArtistInfo, AlbumInfo, SongInfo } from '../types'
import { fetchArtistSongs, fetchArtistAlbums } from '../api/musicApi'
import { usePagedList } from '../composables/usePagedList'
import { useDownloadActions } from '../composables/useDownloadActions'
import SearchResultList from '../components/search/SearchResultList.vue'
import AlbumSearchResult from '../components/search/AlbumSearchResult.vue'
import BatchDownloadBar from '../components/search/BatchDownloadBar.vue'
import LoadMoreButton from '../components/search/LoadMoreButton.vue'

const route = useRoute()
const router = useRouter()
const artist = ref<ArtistInfo | null>(null)
const tab = ref<'songs' | 'albums'>('songs')
const selectedIds = ref<string[]>([])
const routeError = ref('')
const songPage = usePagedList<SongInfo>(song => song.mid)
const albumPage = usePagedList<AlbumInfo>(album => album.id)
const {
    downloadSingle,
    batchDownload
} = useDownloadActions()
let identity = ''
let platform = ''
let albumsStarted = false

watch(() => [route.path, route.query.platform, route.query.id], () => {
    if (route.path !== '/artist') return
    const query = route.query
    if (typeof query.id !== 'string' || !query.id || typeof query.platform !== 'string' || !['qqmusic', 'kuwo'].includes(query.platform)) {
        identity = ''
        songPage.reset()
        albumPage.reset()
        artist.value = null
        routeError.value = '缺少有效的歌手信息，请返回搜索重新选择'
        return
    }
    const nextIdentity = `${query.platform}:${query.id}`
    if (identity === nextIdentity) return
    identity = nextIdentity
    platform = query.platform
    const id = query.id
    const currentPlatform = platform
    const text = (key: string) => typeof query[key] === 'string' ? query[key] as string : ''
    const count = (key: string) => Math.max(0, Number(text(key)) || 0)
    artist.value = {
        id,
        name: text('name'),
        coverUrl: text('cover'),
        alias: text('alias'),
        region: text('region'),
        songCount: count('songs'),
        albumCount: count('albums')
    }
    routeError.value = ''
    selectedIds.value = []
    tab.value = 'songs'
    albumPage.reset()
    albumsStarted = false
    void songPage.start(async page => {
        const result = await fetchArtistSongs(currentPlatform, id, page)
        return {
            items: result.songs,
            total: result.total,
            hasMore: result.has_more
        }
    })
}, { immediate: true })

function showAlbums() {
    tab.value = 'albums'
    if (albumsStarted || !artist.value) return
    albumsStarted = true
    const id = artist.value.id
    const currentPlatform = platform
    void albumPage.start(async page => {
        const result = await fetchArtistAlbums(currentPlatform, id, page)
        return {
            items: result.albums,
            total: result.total,
            hasMore: result.has_more
        }
    })
}

function openAlbum(album: AlbumInfo) {
    router.push({
        path: '/album',
        query: {
            platform,
            id: album.id,
            name: album.name,
            artist: album.artist,
            date: album.publishDate,
            returnTo: route.fullPath
        }
    })
}

function downloadSelected() {
    batchDownload(songPage.items.value.filter(song => selectedIds.value.includes(song.mid)))
}
</script>

<style scoped>
.artist-view {
    display: flex;
    flex-direction: column;
    gap: 20px;
    min-width: 0;
}

.artist-view > .n-button {
    align-self: flex-start;
}

.artist-header {
    display: flex;
    align-items: center;
    gap: 20px;
}

.artist-header img {
    width: 120px;
    height: 120px;
    flex-shrink: 0;
    object-fit: cover;
    border-radius: 50%;
}

.artist-info {
    min-width: 0;
    overflow-wrap: anywhere;
}

.artist-info h2 {
    margin: 0 0 8px;
}

.artist-info p {
    margin: 4px 0;
    color: var(--color-text-secondary);
}

.artist-tabs {
    display: flex;
    gap: 8px;
}

.loading {
    display: flex;
    justify-content: center;
    padding: 40px 0;
}

@media (max-width: 480px) {
    .artist-header {
        gap: 12px;
    }

    .artist-header img {
        width: 80px;
        height: 80px;
    }
}
</style>
