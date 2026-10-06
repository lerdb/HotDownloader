<template>
    <PlaylistHome
        v-if="!isDetail"
        @import="handleImport"
        @open-my="openMyPlaylist"
    />
    <PlaylistDetail
        v-else
        :back-label="backLabel"
        :loading="loading"
        :error="routeError || errorMsg"
        :retryable="!routeError"
        :playlist="playlist"
        :songs="songs"
        :selected-ids="selectedIds"
        :is-all-selected="isAllSelected"
        :is-indeterminate="isIndeterminate"
        :can-monitor="canMonitor"
        @monitor="addMonitor"
        @back="goBack"
        @retry="loadDetail(true)"
        @toggle-all="toggleAll"
        @toggle-select="toggleSelect"
        @download="downloadSingle"
        @click-artist="openRelatedArtist"
        @click-album="openSongAlbum"
        @batch-download="onBatchDownload"
    />
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import PlaylistHome from '../components/playlist/PlaylistHome.vue'
import PlaylistDetail from '../components/playlist/PlaylistDetail.vue'
import { usePlaylistImport } from '../composables/usePlaylistImport'
import { useDownloadActions } from '../composables/useDownloadActions'
import { useMusicNavigation } from '../composables/useMusicNavigation'
import type { PlaylistSearchItem } from '../types'
import { PLATFORMS } from '../config/platforms'
import { isNativeRuntime } from '../api/runtimeApi'

const route = useRoute()
const router = useRouter()
// 缓存页面停用后全局 route 仍会变化；只保存歌单路由自己的查询参数。
const playlistQuery = ref({ ...route.query })
const isDetail = computed(() => playlistQuery.value.id != null)
const routeError = ref('')

// 歌单页被 keep-alive 缓存。从歌手/专辑返回时复用当前详情与歌曲勾选。
let loadedKey = ''

const {
    loading,
    errorMsg,
    playlist,
    songs,
    selectedIds,
    isAllSelected,
    isIndeterminate,
    toggleAll,
    toggleSelect,
    importPlaylist,
    loadCreatedPlaylist,
    reset,
} = usePlaylistImport()
const { downloadSingle, batchDownload } = useDownloadActions()
const canMonitor = computed(
    () =>
        !isNativeRuntime() &&
        playlistQuery.value.platform === 'qqmusic' &&
        !!playlist.value &&
        /^\d+$/.test(playlist.value.id),
)
function addMonitor() {
    if (!canMonitor.value || !playlist.value) return
    const mine = playlistQuery.value.source === 'mine'
    const dirid =
        mine && typeof playlistQuery.value.dirid === 'string'
            ? playlistQuery.value.dirid
            : ''
    void router.push({
        path: '/playlist/monitors',
        query: {
            add: '1',
            name: playlist.value.name,
            playlistId: playlist.value.id,
            source: mine ? (dirid === '201' ? 'liked' : 'created') : 'public',
            dirid,
        },
    })
}
const { openRelatedArtist, openSongAlbum, goBack, backLabel } =
    useMusicNavigation('/playlist')

function handleImport(platform: string, input: string) {
    void router.push({
        path: '/playlist',
        query: { platform, id: input },
        state: { musicReturnTo: '/playlist' },
    })
}

function openMyPlaylist(item: PlaylistSearchItem) {
    void router.push({
        path: '/playlist',
        query: {
            platform: 'qqmusic',
            id: item.id,
            dirid: item.dirid ?? '0',
            source: 'mine',
        },
        state: { musicReturnTo: '/playlist' },
    })
}

async function loadDetail(force = false) {
    if (route.path !== '/playlist' || !isDetail.value) return

    const id = route.query.id
    const platform = route.query.platform
    const mine = route.query.source === 'mine'
    const dirid = route.query.dirid
    const key = route.fullPath
    if (!force && loadedKey === key && (loading.value || playlist.value)) {
        return
    }

    // 新详情开始时使旧请求失效，避免快速切换歌单后出现过期内容。
    reset()
    routeError.value = ''
    loadedKey = key
    if (
        typeof id !== 'string' ||
        !id ||
        typeof platform !== 'string' ||
        !PLATFORMS.some((option) => option.key === platform)
    ) {
        routeError.value = '歌单地址无效，请返回上一页重新选择'
        return
    }
    if (
        mine &&
        (platform !== 'qqmusic' ||
            !/^\d+$/.test(id) ||
            typeof dirid !== 'string' ||
            !/^\d+$/.test(dirid))
    ) {
        routeError.value = '个人歌单地址无效，请返回上一页重新选择'
        return
    }

    if (mine) {
        await loadCreatedPlaylist(id, dirid as string)
    } else {
        await importPlaylist(platform, id)
    }
}

function onBatchDownload() {
    const selectedSongs = songs.value.filter((song) =>
        selectedIds.value.includes(song.mid),
    )
    if (selectedSongs.length) {
        batchDownload(selectedSongs)
    }
}

watch(
    () => route.fullPath,
    () => {
        if (route.path !== '/playlist') return

        playlistQuery.value = { ...route.query }
        if (isDetail.value) {
            void loadDetail()
        } else {
            loadedKey = ''
            reset()
        }
    },
    { immediate: true },
)
</script>
