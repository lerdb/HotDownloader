<template>
    <div class="album-view">
        <n-button
            round
            secondary
            size="small"
            class="back-button"
            @click="goBack"
        >
            <template #icon>
                <AppIcon name="back" :size="16" />
            </template>
            {{ backLabel }}
        </n-button>
        <div v-if="loading" class="loading">
            <n-spin size="medium" description="正在获取专辑全部歌曲…" />
        </div>
        <n-alert v-else-if="error" type="error" title="获取专辑失败">
            {{ error }}
            <n-button size="small" @click="loadAlbum">重试</n-button>
        </n-alert>
        <template v-else-if="album">
            <div class="hero-card">
                <div
                    v-if="album.coverUrl"
                    class="hero-backdrop"
                    :style="{ backgroundImage: `url(${album.coverUrl})` }"
                    aria-hidden="true"
                />
                <div class="hero-scrim" aria-hidden="true" />
                <div class="hero-inner">
                    <img
                        v-if="album.coverUrl"
                        :src="album.coverUrl"
                        class="album-cover"
                        alt="专辑封面"
                    />
                    <div v-else class="album-cover album-cover-fallback">
                        <AppIcon name="disc" :size="44" />
                    </div>
                    <div class="album-info">
                        <span class="hero-type-tag">专辑</span>
                        <h2>{{ album.name || '专辑' }}</h2>
                        <p class="artist-line">
                            <ArtistNames
                                :platform="platform"
                                :artists="album.artists"
                                :fallback="album.artist"
                                @click-artist="openRelatedArtist"
                            />
                        </p>
                        <div class="hero-pills">
                            <span class="meta-pill">
                                {{ album.songCount }} 首歌曲
                            </span>
                            <span v-if="album.publishDate" class="meta-pill">
                                {{ album.publishDate }}
                            </span>
                        </div>
                    </div>
                </div>
            </div>
            <SearchResultList
                v-if="songs.length"
                :songs="songs"
                v-model:selectedIds="selectedIds"
                @download="downloadSingle"
                @click-artist="openRelatedArtist"
                @click-album="openSongAlbum"
            />
            <n-empty v-else description="该专辑暂无可用歌曲" />
            <BatchDownloadBar
                v-if="selectedIds.length"
                :selected-count="selectedIds.length"
                @batch-download="downloadSelected"
            />
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
import AppIcon from '../components/common/AppIcon.vue'
import { useMusicNavigation } from '../composables/useMusicNavigation'

const route = useRoute()
const { openRelatedArtist, openSongAlbum, goBack, backLabel } =
    useMusicNavigation()
const query = { ...route.query }
const platform = typeof query.platform === 'string' ? query.platform : ''
const album = ref<AlbumInfo | null>(null)
const songs = ref<SongInfo[]>([])
const selectedIds = ref<string[]>([])
const loading = ref(false)
const error = ref('')
const { downloadSingle, batchDownload } = useDownloadActions()
async function loadAlbum() {
    if (loading.value) return
    album.value = null
    songs.value = []
    selectedIds.value = []
    error.value = ''
    loading.value = true
    try {
        if (typeof query.platform !== 'string' || typeof query.id !== 'string')
            throw new Error('缺少专辑信息，请返回搜索重新选择')
        const result = await fetchAlbumSongs(query.platform, query.id)
        album.value = {
            ...result.album,
            name:
                result.album.name ||
                (typeof query.name === 'string' ? query.name : ''),
            artist:
                result.album.artist ||
                (typeof query.artist === 'string' ? query.artist : ''),
            publishDate:
                result.album.publishDate ||
                (typeof query.date === 'string' ? query.date : ''),
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
    batchDownload(
        songs.value.filter((song) => selectedIds.value.includes(song.mid)),
    )
}
</script>

<style scoped>
.album-view {
    display: flex;
    flex-direction: column;
    gap: 18px;
    min-width: 0;
}

.back-button {
    align-self: flex-start;
}

.loading {
    display: flex;
    justify-content: center;
    padding: 48px 0;
}

.hero-card {
    position: relative;
    overflow: hidden;
    border-radius: 20px;
    border: 1px solid var(--border-color);
    background: var(--bg-sidebar);
    box-shadow: var(--shadow-md);
}

.hero-backdrop {
    position: absolute;
    inset: -24px;
    background-size: cover;
    background-position: center;
    filter: blur(36px) saturate(140%);
    opacity: 0.25;
}

.hero-scrim {
    position: absolute;
    inset: 0;
    background: linear-gradient(
        135deg,
        var(--bg-glass) 0%,
        var(--bg-glass-strong) 100%
    );
}

.hero-inner {
    position: relative;
    z-index: 1;
    display: flex;
    align-items: center;
    gap: 20px;
    padding: 22px;
}

.album-cover {
    width: 128px;
    height: 128px;
    flex-shrink: 0;
    object-fit: cover;
    border-radius: 14px;
    box-shadow: var(--shadow-md);
}

.album-cover-fallback {
    display: grid;
    place-items: center;
    background: var(--color-primary-soft);
    color: var(--color-primary);
}

.album-info {
    flex: 1;
    min-width: 0;
    overflow-wrap: anywhere;
}

.hero-type-tag {
    display: inline-block;
    padding: 2px 9px;
    border-radius: 999px;
    background: var(--color-primary-soft);
    color: var(--color-primary);
    font-size: 11px;
    font-weight: 600;
    margin-bottom: 6px;
}

.album-info h2 {
    font-size: 22px;
    font-weight: 700;
    margin: 0 0 6px;
}

.artist-line {
    margin: 0;
    font-size: 14px;
    color: var(--color-text-secondary);
}

.hero-pills {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    margin-top: 10px;
}

.meta-pill {
    padding: 3px 10px;
    border-radius: 999px;
    background: var(--bg-surface-subtle);
    color: var(--color-text-secondary);
    font-size: 12px;
    font-weight: 500;
}

@media (max-width: 560px) {
    .hero-inner {
        gap: 14px;
        padding: 16px;
    }

    .album-cover {
        width: 84px;
        height: 84px;
        border-radius: 12px;
    }

    .album-info h2 {
        font-size: 18px;
    }
}
</style>
