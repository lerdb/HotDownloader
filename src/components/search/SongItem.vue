<template>
    <div
        class="song-item"
        :class="{
            'is-selected': selected,
            'is-playing': isCurrentSong,
        }"
    >
        <n-checkbox
            :checked="selected"
            @update:checked="$emit('toggleSelect', $event)"
        />
        <button
            type="button"
            class="cover-wrapper"
            :aria-label="
                isCurrentSong && player.status === 'playing'
                    ? `进入 ${song.title} 播放页`
                    : `试听 ${song.title}`
            "
            @click.stop="handlePreviewCover"
        >
            <img
                v-if="coverUrl"
                :src="coverUrl"
                class="cover"
                alt="封面"
                loading="lazy"
            />
            <div v-else-if="coverLoading" class="cover placeholder" />
            <div v-else class="cover placeholder default">
                <AppIcon name="music" :size="20" />
            </div>
            <span class="cover-play-overlay" aria-hidden="true">
                <AppIcon
                    :name="
                        isCurrentSong && player.status === 'playing'
                            ? 'pause'
                            : 'play'
                    "
                    :size="18"
                />
            </span>
        </button>

        <div class="info">
            <div class="title" :title="song.title">{{ song.title }}</div>
            <div class="subtitle-row">
                <span class="subtitle-artist">
                    <ArtistNames
                        :platform="song.platform"
                        :artists="song.artists"
                        :fallback="song.artist"
                        @click-artist="
                            (platform, artist) =>
                                $emit('click-artist', platform, artist)
                        "
                    />
                </span>
                <template v-if="song.album">
                    <span class="subtitle-dot" aria-hidden="true">·</span>
                    <n-button
                        v-if="albumId"
                        text
                        size="small"
                        class="album-link"
                        @click.stop="$emit('click-album', song)"
                    >
                        {{ song.album }}
                    </n-button>
                    <span v-else class="album-plain">{{ song.album }}</span>
                </template>
            </div>
            <div v-if="sortedQualities.length" class="quality-tags">
                <span
                    v-for="q in sortedQualities.slice(0, 4)"
                    :key="q.quality"
                    class="quality-chip"
                    :class="{ 'is-lossless': isHighQuality(q.quality) }"
                >
                    {{ q.quality }}
                </span>
                <span v-if="sortedQualities.length > 4" class="quality-chip">
                    +{{ sortedQualities.length - 4 }}
                </span>
            </div>
        </div>

        <div class="song-actions">
            <n-button
                size="small"
                round
                :secondary="!isCurrentSong"
                :loading="isCurrentSong && player.status === 'loading'"
                :type="isCurrentSong ? 'primary' : 'default'"
                @click.stop="handlePreviewAction"
            >
                <template #icon>
                    <AppIcon
                        :name="
                            isCurrentSong && player.status === 'playing'
                                ? 'pause'
                                : 'play'
                        "
                        :size="14"
                    />
                </template>
                {{
                    isCurrentSong && player.status === 'playing'
                        ? '暂停'
                        : '试听'
                }}
            </n-button>
            <n-button
                size="small"
                round
                secondary
                type="primary"
                class="download-btn"
                @click="$emit('download', song)"
            >
                <template #icon>
                    <AppIcon name="download" :size="14" />
                </template>
                下载
            </n-button>
        </div>
    </div>
</template>

<script setup lang="ts">
import { computed, onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { NCheckbox, NButton } from 'naive-ui'
import type { ArtistReference, SongInfo } from '../../types'
import { ALL_QUALITY_ORDER } from '../../types'
import { fetchCover } from '../../api/musicApi'
import ArtistNames from './ArtistNames.vue'
import { getMusicEntityId } from '../../utils/music'
import { usePlayerStore } from '../../stores/playerStore'
import AppIcon from '../common/AppIcon.vue'

const router = useRouter()
const player = usePlayerStore()

const props = defineProps<{
    song: SongInfo
    selected: boolean
}>()

defineEmits<{
    (e: 'toggleSelect', selected: boolean): void
    (e: 'download', song: SongInfo): void
    (e: 'click-artist', platform: string, artist: ArtistReference): void
    (e: 'click-album', song: SongInfo): void
}>()

const isCurrentSong = computed(() => player.isCurrent(props.song))

function handlePreviewCover() {
    if (!isCurrentSong.value || player.status !== 'playing') {
        void player.play(props.song)
    }
    void router.push('/player')
}

function handlePreviewAction() {
    if (isCurrentSong.value && player.status === 'playing') {
        void player.play(props.song)
        return
    }
    void player.play(props.song)
    void router.push('/player')
}

const albumId = computed(() =>
    getMusicEntityId(
        props.song.platform,
        props.song.albumId,
        props.song.albumMid,
    ),
)

// 按品质从高到低排序
const sortedQualities = computed(() => {
    return [...props.song.qualities].sort((a, b) => {
        const ia = ALL_QUALITY_ORDER.indexOf(a.quality)
        const ib = ALL_QUALITY_ORDER.indexOf(b.quality)
        const idxA = ia === -1 ? -1 : ia
        const idxB = ib === -1 ? -1 : ib
        return idxB - idxA
    })
})

function isHighQuality(quality: string): boolean {
    const q = quality.toLowerCase()
    return (
        q.includes('flac') ||
        q.includes('hires') ||
        q.includes('ape') ||
        q.includes('母带') ||
        q.includes('全景') ||
        q.includes('环绕')
    )
}

// 优先展示歌曲自带的封面，缺少地址时按需请求。
const coverUrl = ref<string>('')
const coverLoading = ref(false)

async function loadCoverIfNeeded() {
    if (props.song.coverUrl) {
        coverUrl.value = props.song.coverUrl
        return
    }
    if (props.song.platform !== 'kuwo' || !props.song.id) return
    const expectedId = props.song.id
    coverLoading.value = true
    try {
        const url = await fetchCover('kuwo', expectedId)
        if (props.song.id === expectedId) {
            coverUrl.value = url
        }
    } catch {
        // 加载失败保持占位
    } finally {
        coverLoading.value = false
    }
}

onMounted(() => {
    loadCoverIfNeeded()
})

watch(
    () => props.song.id,
    () => {
        coverUrl.value = props.song.coverUrl
        loadCoverIfNeeded()
    },
)
</script>

<style scoped>
.song-item {
    display: flex;
    align-items: center;
    gap: 14px;
    min-width: 0;
    padding: 12px 14px;
    background-color: var(--bg-sidebar);
    border: 1px solid var(--border-color);
    border-radius: 14px;
    transition:
        background-color 0.18s ease,
        border-color 0.18s ease,
        box-shadow 0.18s ease,
        transform 0.15s ease;
}

.song-item:hover {
    background-color: var(--bg-elevated);
    border-color: var(--border-color-strong);
    box-shadow: var(--shadow-sm);
}

.song-item.is-selected {
    border-color: var(--color-primary);
    background-color: var(--color-primary-soft);
}

.song-item.is-playing {
    border-color: var(--color-primary);
}

.cover-wrapper {
    position: relative;
    width: 52px;
    height: 52px;
    flex-shrink: 0;
    padding: 0;
    border: none;
    background: transparent;
    border-radius: 10px;
    overflow: hidden;
    cursor: pointer;
}

.cover {
    width: 52px;
    height: 52px;
    border-radius: 10px;
    object-fit: cover;
    display: block;
}

.cover.placeholder {
    background-color: var(--bg-surface-subtle);
}

.cover.placeholder.default {
    display: grid;
    place-items: center;
    color: var(--color-text-secondary);
}

.cover-play-overlay {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: rgba(0, 0, 0, 0.45);
    color: #ffffff;
    opacity: 0;
    transition: opacity 0.18s ease;
}

.cover-wrapper:hover .cover-play-overlay,
.song-item.is-playing .cover-play-overlay {
    opacity: 1;
}

.info {
    flex: 1;
    min-width: 0;
    overflow: hidden;
}

.title {
    font-size: 15px;
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--color-text);
    line-height: 1.4;
}

.song-item.is-playing .title {
    color: var(--color-primary);
}

.subtitle-row {
    display: flex;
    align-items: center;
    gap: 5px;
    margin-top: 3px;
    font-size: 13px;
    color: var(--color-text-secondary);
    min-width: 0;
}

.subtitle-artist,
.album-plain {
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}

.subtitle-dot {
    flex-shrink: 0;
    opacity: 0.6;
}

.album-link {
    font: inherit;
    min-width: 0;
    max-width: 48%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    vertical-align: baseline;
}

.quality-tags {
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
    margin-top: 6px;
}

.quality-chip {
    display: inline-flex;
    align-items: center;
    padding: 1px 8px;
    border-radius: 999px;
    font-size: 11px;
    font-weight: 500;
    background: var(--bg-surface-subtle);
    color: var(--color-text-secondary);
}

.quality-chip.is-lossless {
    background: var(--color-primary-soft);
    color: var(--color-primary);
    font-weight: 600;
}

.download-btn {
    flex-shrink: 0;
}

.song-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
}

/* 手机保留封面和操作入口，长歌名在剩余空间内省略 */
@media (max-width: 767px) {
    .song-item {
        gap: 10px;
        padding: 10px 10px;
    }

    .cover-wrapper,
    .cover {
        width: 48px;
        height: 48px;
    }

    .song-actions {
        flex-direction: column;
        gap: 6px;
    }

    .song-actions :deep(.n-button) {
        min-height: 36px;
    }
}
</style>
