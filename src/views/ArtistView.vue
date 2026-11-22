<template>
    <div class="artist-view">
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
        <n-alert v-if="routeError" type="error" title="无法打开歌手">{{
            routeError
        }}</n-alert>
        <template v-else-if="artist">
            <div class="hero-card">
                <div
                    v-if="artist.coverUrl"
                    class="hero-backdrop"
                    :style="{ backgroundImage: `url(${artist.coverUrl})` }"
                    aria-hidden="true"
                />
                <div class="hero-scrim" aria-hidden="true" />
                <div class="hero-inner">
                    <img
                        v-if="artist.coverUrl"
                        :src="artist.coverUrl"
                        class="artist-avatar"
                        alt="歌手头像"
                    />
                    <div v-else class="artist-avatar artist-avatar-fallback">
                        <AppIcon name="artist" :size="44" />
                    </div>
                    <div class="artist-info">
                        <span class="hero-type-tag">歌手</span>
                        <h2>{{ artist.name || '歌手' }}</h2>
                        <p
                            v-if="artist.alias || artist.region"
                            class="artist-sub"
                        >
                            {{
                                [artist.alias, artist.region]
                                    .filter(Boolean)
                                    .join(' · ')
                            }}
                        </p>
                        <div class="hero-pills">
                            <span class="meta-pill">
                                {{ songPage.total.value ?? artist.songCount }}
                                首歌曲
                            </span>
                            <span class="meta-pill">
                                {{ albumPage.total.value ?? artist.albumCount }}
                                张专辑
                            </span>
                        </div>
                    </div>
                </div>
            </div>

            <div class="artist-tabs" role="tablist">
                <button
                    type="button"
                    role="tab"
                    :aria-selected="tab === 'songs'"
                    class="tab-pill"
                    :class="{ 'is-active': tab === 'songs' }"
                    @click="tab = 'songs'"
                >
                    <AppIcon name="music" :size="15" />
                    <span>热门歌曲</span>
                </button>
                <button
                    type="button"
                    role="tab"
                    :aria-selected="tab === 'albums'"
                    class="tab-pill"
                    :class="{ 'is-active': tab === 'albums' }"
                    @click="showAlbums"
                >
                    <AppIcon name="disc" :size="15" />
                    <span>全部专辑</span>
                </button>
            </div>

            <template v-if="tab === 'songs'">
                <n-alert
                    v-if="songPage.error.value"
                    type="error"
                    title="获取歌手歌曲失败"
                >
                    {{ songPage.error.value }}
                    <n-button size="small" @click="songPage.loadMore"
                        >重试</n-button
                    >
                </n-alert>
                <div
                    v-if="songPage.loading.value && !songPage.loaded.value"
                    class="loading"
                >
                    <n-spin />
                </div>
                <SearchResultList
                    v-if="songPage.items.value.length"
                    :songs="songPage.items.value"
                    v-model:selectedIds="selectedIds"
                    @download="downloadSingle"
                    @click-artist="openRelatedArtist"
                    @click-album="openSongAlbum"
                />
                <n-empty
                    v-else-if="songPage.loaded.value && !songPage.error.value"
                    description="暂无可用歌曲"
                />
                <LoadMoreButton
                    v-if="songPage.hasMore.value"
                    :loading="songPage.loading.value"
                    :disabled="songPage.loading.value"
                    @click="songPage.loadMore"
                />
                <BatchDownloadBar
                    v-if="selectedIds.length"
                    :selected-count="selectedIds.length"
                    @batch-download="downloadSelected"
                />
            </template>
            <template v-else>
                <n-alert
                    v-if="albumPage.error.value"
                    type="error"
                    title="获取歌手专辑失败"
                >
                    {{ albumPage.error.value }}
                    <n-button size="small" @click="albumPage.loadMore"
                        >重试</n-button
                    >
                </n-alert>
                <div
                    v-if="albumPage.loading.value && !albumPage.loaded.value"
                    class="loading"
                >
                    <n-spin />
                </div>
                <AlbumSearchResult
                    v-if="albumPage.loaded.value"
                    :albums="albumPage.items.value"
                    :platform="platform"
                    :has-more="false"
                    :loading-more="false"
                    @click-album="(album) => openAlbum(platform, album)"
                    @click-artist="openRelatedArtist"
                />
                <LoadMoreButton
                    v-if="albumPage.hasMore.value"
                    :loading="albumPage.loading.value"
                    :disabled="albumPage.loading.value"
                    @click="albumPage.loadMore"
                />
            </template>
        </template>
    </div>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRoute } from 'vue-router'
import { NAlert, NButton, NEmpty, NSpin } from 'naive-ui'
import type { ArtistInfo, AlbumInfo, SongInfo } from '../types'
import { fetchArtistSongs, fetchArtistAlbums } from '../api/musicApi'
import { usePagedList } from '../composables/usePagedList'
import { useDownloadActions } from '../composables/useDownloadActions'
import SearchResultList from '../components/search/SearchResultList.vue'
import AlbumSearchResult from '../components/search/AlbumSearchResult.vue'
import BatchDownloadBar from '../components/search/BatchDownloadBar.vue'
import LoadMoreButton from '../components/search/LoadMoreButton.vue'
import AppIcon from '../components/common/AppIcon.vue'
import { useMusicNavigation } from '../composables/useMusicNavigation'

const route = useRoute()
const { openAlbum, openRelatedArtist, openSongAlbum, goBack, backLabel } =
    useMusicNavigation()
const query = { ...route.query }
const platform = typeof query.platform === 'string' ? query.platform : ''
const artist = ref<ArtistInfo | null>(null)
const tab = ref<'songs' | 'albums'>('songs')
const selectedIds = ref<string[]>([])
const routeError = ref('')
const songPage = usePagedList<SongInfo>((song) => song.mid)
const albumPage = usePagedList<AlbumInfo>((album) => album.id)
const { downloadSingle, batchDownload } = useDownloadActions()
let albumsStarted = false

onMounted(() => {
    if (
        typeof query.id !== 'string' ||
        !query.id ||
        !['qqmusic', 'kuwo', 'kugou', 'netease'].includes(platform)
    ) {
        routeError.value = '缺少有效的歌手信息，请返回搜索重新选择'
        return
    }
    const id = query.id
    const text = (key: string) =>
        typeof query[key] === 'string' ? (query[key] as string) : ''
    const count = (key: string) => Math.max(0, Number(text(key)) || 0)
    artist.value = {
        id,
        name: text('name'),
        coverUrl: text('cover'),
        alias: text('alias'),
        region: text('region'),
        songCount: count('songs'),
        albumCount: count('albums'),
    }
    void songPage.start(async (page) => {
        const result = await fetchArtistSongs(platform, id, page)
        return {
            items: result.songs,
            total: result.total,
            hasMore: result.has_more,
        }
    })
})

function showAlbums() {
    tab.value = 'albums'
    if (albumsStarted || !artist.value) return
    albumsStarted = true
    const id = artist.value.id
    void albumPage.start(async (page) => {
        const result = await fetchArtistAlbums(platform, id, page)
        return {
            items: result.albums,
            total: result.total,
            hasMore: result.has_more,
        }
    })
}

function downloadSelected() {
    batchDownload(
        songPage.items.value.filter((song) =>
            selectedIds.value.includes(song.mid),
        ),
    )
}
</script>

<style scoped>
.artist-view {
    display: flex;
    flex-direction: column;
    gap: 18px;
    min-width: 0;
}

.back-button {
    align-self: flex-start;
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

.artist-avatar {
    width: 124px;
    height: 124px;
    flex-shrink: 0;
    object-fit: cover;
    border-radius: 50%;
    box-shadow: var(--shadow-md);
    border: 2px solid var(--border-color-strong);
}

.artist-avatar-fallback {
    display: grid;
    place-items: center;
    background: var(--color-primary-soft);
    color: var(--color-primary);
}

.artist-info {
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

.artist-info h2 {
    font-size: 22px;
    font-weight: 700;
    margin: 0 0 4px;
}

.artist-sub {
    margin: 2px 0 0;
    font-size: 13px;
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

.artist-tabs {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
}

.tab-pill {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    min-height: 36px;
    padding: 6px 16px;
    border-radius: 999px;
    border: 1px solid var(--border-color);
    background: var(--bg-sidebar);
    color: var(--color-text-secondary);
    font: inherit;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition:
        background-color 0.18s ease,
        color 0.18s ease,
        border-color 0.18s ease;
}

.tab-pill.is-active {
    background: var(--color-primary-soft);
    color: var(--color-primary);
    border-color: var(--color-primary);
    font-weight: 600;
}

.loading {
    display: flex;
    justify-content: center;
    padding: 40px 0;
}

@media (max-width: 560px) {
    .hero-inner {
        gap: 14px;
        padding: 16px;
    }

    .artist-avatar {
        width: 82px;
        height: 82px;
    }

    .artist-info h2 {
        font-size: 18px;
    }
}
</style>
