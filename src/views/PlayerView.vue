<template>
    <div class="player-page">
        <!-- SimpMusic / Apple Music 风格动态模糊背景 -->
        <div
            v-if="displayCoverUrl"
            class="ambient-backdrop"
            :style="{ backgroundImage: `url(${displayCoverUrl})` }"
            aria-hidden="true"
        />
        <div class="ambient-glow" aria-hidden="true" />
        <div class="ambient-scrim" aria-hidden="true" />

        <div class="player-shell">
            <!-- 顶部栏 -->
            <header class="player-topbar">
                <button
                    type="button"
                    class="top-pill-btn"
                    aria-label="返回上一页"
                    @click="goBack"
                >
                    <AppIcon name="collapse" :size="18" />
                    <span>返回</span>
                </button>

                <div class="top-center">
                    <div class="mobile-mode-switch" role="tablist" aria-label="播放视图切换">
                        <button
                            type="button"
                            role="tab"
                            :aria-selected="mobileTab === 'cover'"
                            class="mode-tab"
                            :class="{ 'is-active': mobileTab === 'cover' }"
                            @click="mobileTab = 'cover'"
                        >
                            封面
                        </button>
                        <button
                            type="button"
                            role="tab"
                            :aria-selected="mobileTab === 'lyrics'"
                            class="mode-tab"
                            :class="{ 'is-active': mobileTab === 'lyrics' }"
                            @click="mobileTab = 'lyrics'"
                        >
                            歌词
                        </button>
                    </div>

                    <div v-if="player.song" class="desktop-status-pill">
                        <span
                            class="status-dot"
                            :class="{ 'is-playing': player.status === 'playing' }"
                            aria-hidden="true"
                        />
                        <span>{{ statusText }}</span>
                        <span v-if="platformLabel" class="status-sep">·</span>
                        <span v-if="platformLabel">{{ platformLabel }}</span>
                        <span v-if="player.source" class="quality-badge">
                            {{ player.source.quality }}
                        </span>
                    </div>
                </div>

                <div v-if="player.song" class="top-actions">
                    <button
                        type="button"
                        class="top-icon-btn"
                        aria-label="下载当前歌曲"
                        title="下载当前歌曲"
                        @click="downloadCurrent"
                    >
                        <AppIcon name="download" :size="18" />
                    </button>
                    <button
                        type="button"
                        class="top-icon-btn"
                        aria-label="停止并退出播放器"
                        title="关闭播放器"
                        @click="stopAndBack"
                    >
                        <AppIcon name="close" :size="18" />
                    </button>
                </div>
                <div v-else class="top-actions-placeholder" aria-hidden="true" />
            </header>

            <!-- 无正在播放歌曲时的兜底提示 -->
            <div v-if="!player.song" class="empty-player">
                <div class="empty-disc">
                    <AppIcon name="disc" :size="48" />
                </div>
                <h2>暂无正在播放的歌曲</h2>
                <p>从搜索、歌单或专辑列表选择一首歌曲点击“试听”即可开始</p>
                <button
                    type="button"
                    class="empty-back-btn"
                    @click="router.replace('/search')"
                >
                    前往发现音乐
                </button>
            </div>

            <!-- 播放器主体 -->
            <main v-else class="player-body">
                <!-- 左侧（桌面） / 封面视图（移动端）：封面、歌曲信息、进度条与播放控制 -->
                <section
                    class="stage-left"
                    :class="{ 'is-mobile-hidden': mobileTab === 'lyrics' }"
                >
                    <div class="artwork-frame">
                        <img
                            v-if="displayCoverUrl"
                            :src="displayCoverUrl"
                            :alt="player.song.title"
                            class="artwork-image"
                            :class="{
                                'is-playing': player.status === 'playing',
                            }"
                        />
                        <div v-else class="artwork-image artwork-fallback">
                            <AppIcon name="disc" :size="84" />
                        </div>
                    </div>

                    <div class="track-meta">
                        <div class="track-heading">
                            <h1 class="track-title" :title="player.song.title">
                                {{ player.song.title }}
                            </h1>
                            <div class="track-sub">
                                <span class="artist-wrap">
                                    <ArtistNames
                                        :platform="player.song.platform"
                                        :artists="player.song.artists"
                                        :fallback="player.song.artist"
                                        @click-artist="openRelatedArtist"
                                    />
                                </span>
                                <template v-if="player.song.album">
                                    <span class="sub-dot">·</span>
                                    <button
                                        v-if="albumId"
                                        type="button"
                                        class="album-btn"
                                        @click="openSongAlbum(player.song)"
                                    >
                                        {{ player.song.album }}
                                    </button>
                                    <span v-else class="album-text">
                                        {{ player.song.album }}
                                    </span>
                                </template>
                            </div>
                        </div>

                        <div class="mobile-badges">
                            <span v-if="platformLabel" class="mobile-badge">
                                {{ platformLabel }}
                            </span>
                            <span v-if="player.source" class="mobile-badge is-accent">
                                {{ player.source.quality }}
                            </span>
                        </div>
                    </div>

                    <!-- 移动端封面模式下显示两行同步歌词预览，点击切到完整歌词 -->
                    <button
                        type="button"
                        class="mobile-lyric-preview"
                        aria-label="点击查看完整同步歌词"
                        @click="mobileTab = 'lyrics'"
                    >
                        <template v-if="lyricStore.loading">
                            <span class="preview-muted">正在加载歌词…</span>
                        </template>
                        <template v-else-if="lyricStore.instrumental">
                            <span class="preview-muted">纯音乐，请欣赏</span>
                        </template>
                        <template v-else-if="lyricStore.activeLine">
                            <span class="preview-current">
                                {{ lyricStore.activeLine.text }}
                            </span>
                            <span
                                v-if="
                                    lyricStore.activeLine.translation ||
                                    lyricStore.nextLine?.text
                                "
                                class="preview-next"
                            >
                                {{
                                    lyricStore.activeLine.translation ||
                                    lyricStore.nextLine?.text
                                }}
                            </span>
                        </template>
                        <template v-else-if="lyricStore.lines.length">
                            <span class="preview-current">
                                {{ lyricStore.lines[0].text }}
                            </span>
                            <span class="preview-next">点击查看完整歌词</span>
                        </template>
                        <template v-else>
                            <span class="preview-muted">点击查看歌词</span>
                        </template>
                    </button>

                    <!-- 播放进度条 -->
                    <div class="seek-section">
                        <input
                            class="seek-slider"
                            type="range"
                            min="0"
                            :max="player.duration || 1"
                            step="0.1"
                            :value="player.currentTime"
                            :disabled="player.duration <= 0"
                            :style="sliderBackground(progressPercent)"
                            aria-label="播放进度"
                            :aria-valuetext="`${formatTime(player.currentTime)} / ${formatTime(player.duration)}`"
                            @input="player.seek(inputValue($event))"
                        />
                        <div class="time-row">
                            <span>{{ formatTime(player.currentTime) }}</span>
                            <span>{{ formatTime(player.duration) }}</span>
                        </div>
                    </div>

                    <!-- 播放控制区 -->
                    <div class="transport-controls">
                        <button
                            type="button"
                            class="transport-btn"
                            :disabled="player.duration <= 0"
                            aria-label="后退 10 秒"
                            title="后退 10 秒"
                            @click="seekBy(-10)"
                        >
                            <AppIcon name="seek-back" :size="22" />
                            <span class="seek-step">10s</span>
                        </button>

                        <button
                            type="button"
                            class="primary-play-btn"
                            :disabled="player.status === 'loading'"
                            :aria-label="playButtonLabel"
                            @click="player.toggle()"
                        >
                            <span
                                v-if="player.status === 'loading'"
                                class="btn-spinner"
                                aria-hidden="true"
                            />
                            <AppIcon
                                v-else
                                :name="
                                    player.status === 'playing'
                                        ? 'pause'
                                        : 'play'
                                "
                                :size="30"
                            />
                        </button>

                        <button
                            type="button"
                            class="transport-btn"
                            :disabled="player.duration <= 0"
                            aria-label="前进 10 秒"
                            title="前进 10 秒"
                            @click="seekBy(10)"
                        >
                            <AppIcon name="seek-forward" :size="22" />
                            <span class="seek-step">10s</span>
                        </button>
                    </div>

                    <!-- 音量条 -->
                    <div class="volume-row">
                        <button
                            type="button"
                            class="mute-btn"
                            :aria-label="
                                player.volume === 0 ? '取消静音' : '静音'
                            "
                            @click="toggleMute"
                        >
                            <AppIcon
                                :name="
                                    player.volume === 0
                                        ? 'volume-mute'
                                        : 'volume'
                                "
                                :size="17"
                            />
                        </button>
                        <input
                            class="volume-slider"
                            type="range"
                            min="0"
                            max="1"
                            step="0.01"
                            :value="player.volume"
                            :style="sliderBackground(player.volume * 100)"
                            aria-label="音量"
                            @input="player.setVolume(inputValue($event))"
                        />
                    </div>

                    <p v-if="player.error" class="playback-feedback is-error" role="alert">
                        {{ player.error }}
                    </p>
                    <p
                        v-else-if="
                            player.status === 'loading' || player.buffering
                        "
                        class="playback-feedback"
                        role="status"
                    >
                        {{
                            player.source
                                ? '正在缓冲音频…'
                                : '正在获取试听链接…'
                        }}
                    </p>
                </section>

                <!-- 右侧（桌面） / 歌词视图（移动端）：同步歌词舞台 -->
                <section
                    class="stage-right"
                    :class="{ 'is-mobile-hidden': mobileTab === 'cover' }"
                >
                    <div v-if="mobileTab === 'lyrics'" class="mobile-lyrics-mini">
                        <img
                            v-if="displayCoverUrl"
                            :src="displayCoverUrl"
                            :alt="player.song.title"
                            class="mini-thumb"
                        />
                        <div v-else class="mini-thumb mini-thumb-fallback">
                            <AppIcon name="disc" :size="18" />
                        </div>
                        <div class="mini-thumb-info">
                            <div class="mini-thumb-title">
                                {{ player.song.title }}
                            </div>
                            <div class="mini-thumb-artist">
                                {{ player.song.artist }}
                            </div>
                        </div>
                        <button
                            type="button"
                            class="mini-play-pill"
                            :disabled="player.status === 'loading'"
                            :aria-label="playButtonLabel"
                            @click="player.toggle()"
                        >
                            <AppIcon
                                :name="
                                    player.status === 'playing'
                                        ? 'pause'
                                        : 'play'
                                "
                                :size="18"
                            />
                        </button>
                    </div>

                    <LyricStage class="stage-lyrics-box" />

                    <!-- 移动端歌词视图底部保留轻量化进度条，方便边看歌词边拖动 -->
                    <div v-if="mobileTab === 'lyrics'" class="mobile-lyrics-seek">
                        <span class="mini-time">{{
                            formatTime(player.currentTime)
                        }}</span>
                        <input
                            class="seek-slider"
                            type="range"
                            min="0"
                            :max="player.duration || 1"
                            step="0.1"
                            :value="player.currentTime"
                            :disabled="player.duration <= 0"
                            :style="sliderBackground(progressPercent)"
                            aria-label="播放进度"
                            @input="player.seek(inputValue($event))"
                        />
                        <span class="mini-time">{{
                            formatTime(player.duration)
                        }}</span>
                    </div>
                </section>
            </main>
        </div>
    </div>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { fetchCover } from '../api/musicApi'
import { useDownloadActions } from '../composables/useDownloadActions'
import { useMusicNavigation } from '../composables/useMusicNavigation'
import { PLATFORMS } from '../config/platforms'
import { useLyricStore } from '../stores/lyricStore'
import { usePlayerStore } from '../stores/playerStore'
import { getMusicEntityId } from '../utils/music'
import AppIcon from '../components/common/AppIcon.vue'
import LyricStage from '../components/player/LyricStage.vue'
import ArtistNames from '../components/search/ArtistNames.vue'

const router = useRouter()
const player = usePlayerStore()
const lyricStore = useLyricStore()
const { downloadSingle } = useDownloadActions()
const { openRelatedArtist, openSongAlbum } = useMusicNavigation('/search')

const mobileTab = ref<'cover' | 'lyrics'>('cover')
const lastNonZeroVolume = ref(0.8)
const fetchedCoverUrl = ref('')
let coverRequestId = 0

watch(
    () => player.song,
    async (song) => {
        const request = ++coverRequestId
        fetchedCoverUrl.value = ''
        if (!song || song.coverUrl || song.platform !== 'kuwo' || !song.id) {
            return
        }
        try {
            const url = await fetchCover('kuwo', song.id)
            if (request === coverRequestId) {
                fetchedCoverUrl.value = url
            }
        } catch {
            // 封面获取失败时保持默认占位图
        }
    },
    { immediate: true },
)

const displayCoverUrl = computed(
    () => player.song?.coverUrl || fetchedCoverUrl.value,
)

const albumId = computed(() =>
    player.song
        ? getMusicEntityId(
              player.song.platform,
              player.song.albumId,
              player.song.albumMid,
          )
        : '',
)

const platformLabel = computed(() => {
    const key = player.song?.platform
    if (!key) return ''
    return PLATFORMS.find((p) => p.key === key)?.label ?? key
})

const progressPercent = computed(() => {
    if (player.duration <= 0) return 0
    return Math.max(
        0,
        Math.min(100, (player.currentTime / player.duration) * 100),
    )
})

const playButtonLabel = computed(() =>
    player.status === 'playing'
        ? '暂停'
        : player.status === 'error'
          ? '重试'
          : '播放',
)

const statusText = computed(
    () =>
        ({
            idle: '准备就绪',
            loading: '加载中',
            playing: '正在试听',
            paused: '已暂停',
            ended: '试听结束',
            error: '播放出错',
        })[player.status],
)

function sliderBackground(percent: number) {
    const clamped = Math.max(0, Math.min(100, percent))
    return {
        background: `linear-gradient(to right, #ffffff 0%, #ffffff ${clamped}%, rgba(255, 255, 255, 0.22) ${clamped}%, rgba(255, 255, 255, 0.22) 100%)`,
    }
}

function seekBy(deltaSeconds: number) {
    if (player.duration <= 0) return
    player.seek(player.currentTime + deltaSeconds)
}

function toggleMute() {
    if (player.volume > 0) {
        lastNonZeroVolume.value = player.volume
        player.setVolume(0)
    } else {
        player.setVolume(lastNonZeroVolume.value || 0.8)
    }
}

function downloadCurrent() {
    if (!player.song) return
    void downloadSingle(player.song)
}

function goBack() {
    if (window.history.length > 1) {
        router.back()
    } else {
        void router.replace('/search')
    }
}

function stopAndBack() {
    player.stop()
    goBack()
}

function inputValue(event: Event): number {
    return Number((event.target as HTMLInputElement).value)
}

function formatTime(value: number): string {
    const seconds = Number.isFinite(value) ? Math.max(0, Math.floor(value)) : 0
    return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`
}
</script>

<style scoped>
.player-page {
    position: relative;
    width: 100%;
    height: 100%;
    min-height: 0;
    overflow: hidden;
    background: #0a0d14;
    color: #ffffff;
}

.ambient-backdrop {
    position: absolute;
    inset: -48px;
    background-size: cover;
    background-position: center;
    filter: blur(84px) saturate(185%) brightness(0.65);
    opacity: 0.65;
    transform: scale(1.18);
    transition: background-image 0.5s ease;
    pointer-events: none;
}

.ambient-glow {
    position: absolute;
    inset: 0;
    background: radial-gradient(
        circle at 25% 25%,
        var(--color-primary-soft),
        transparent 62%
    );
    pointer-events: none;
}

.ambient-scrim {
    position: absolute;
    inset: 0;
    background: linear-gradient(
        180deg,
        rgba(8, 10, 16, 0.42) 0%,
        rgba(8, 10, 16, 0.68) 55%,
        rgba(8, 10, 16, 0.88) 100%
    );
    pointer-events: none;
}

.player-shell {
    position: relative;
    z-index: 1;
    width: 100%;
    height: 100%;
    max-width: 1240px;
    margin: 0 auto;
    display: flex;
    flex-direction: column;
    padding: calc(16px + var(--safe-area-top))
        calc(28px + var(--safe-area-right))
        calc(22px + var(--safe-area-bottom))
        calc(28px + var(--safe-area-left));
    min-height: 0;
}

/* 顶部栏 */
.player-topbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    flex-shrink: 0;
    margin-bottom: 14px;
}

.top-pill-btn {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 38px;
    padding: 0 14px;
    border-radius: 999px;
    border: 1px solid rgba(255, 255, 255, 0.16);
    background: rgba(255, 255, 255, 0.08);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    color: #ffffff;
    font: inherit;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition: background-color 0.18s ease;
}

.top-pill-btn:hover {
    background: rgba(255, 255, 255, 0.16);
}

.top-center {
    display: flex;
    align-items: center;
    justify-content: center;
    min-width: 0;
}

.desktop-status-pill {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    height: 34px;
    padding: 0 14px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.12);
    font-size: 12px;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.85);
}

.status-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.45);
}

.status-dot.is-playing {
    background: var(--color-primary);
    box-shadow: 0 0 10px var(--color-primary);
}

.status-sep {
    opacity: 0.45;
}

.quality-badge {
    padding: 1px 8px;
    border-radius: 999px;
    background: rgba(255, 255, 255, 0.14);
    color: #ffffff;
    font-size: 11px;
    font-weight: 600;
}

.mobile-mode-switch {
    display: none;
}

.top-actions {
    display: flex;
    align-items: center;
    gap: 8px;
}

.top-actions-placeholder {
    width: 84px;
}

.top-icon-btn {
    width: 38px;
    height: 38px;
    border-radius: 999px;
    border: 1px solid rgba(255, 255, 255, 0.16);
    background: rgba(255, 255, 255, 0.08);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    color: #ffffff;
    display: grid;
    place-items: center;
    cursor: pointer;
    transition: background-color 0.18s ease;
}

.top-icon-btn:hover {
    background: rgba(255, 255, 255, 0.18);
}

/* 空状态 */
.empty-player {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    text-align: center;
    padding: 24px;
}

.empty-disc {
    width: 88px;
    height: 88px;
    border-radius: 26px;
    background: rgba(255, 255, 255, 0.08);
    display: grid;
    place-items: center;
    color: rgba(255, 255, 255, 0.8);
    margin-bottom: 4px;
}

.empty-player h2 {
    margin: 0;
    font-size: 20px;
    font-weight: 700;
}

.empty-player p {
    margin: 0;
    font-size: 13px;
    color: rgba(255, 255, 255, 0.65);
}

.empty-back-btn {
    margin-top: 8px;
    padding: 10px 22px;
    border-radius: 999px;
    border: none;
    background: #ffffff;
    color: #0f172a;
    font: inherit;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
}

/* 双栏主舞台 */
.player-body {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(300px, 420px) minmax(0, 1fr);
    gap: 52px;
    align-items: center;
}

.stage-left {
    display: flex;
    flex-direction: column;
    align-items: center;
    min-width: 0;
    width: 100%;
    max-height: 100%;
}

.artwork-frame {
    width: min(350px, 34vw, 42vh);
    aspect-ratio: 1 / 1;
    margin-bottom: 22px;
    flex-shrink: 0;
}

.artwork-image {
    width: 100%;
    height: 100%;
    border-radius: 22px;
    object-fit: cover;
    display: block;
    box-shadow: 0 24px 56px rgba(0, 0, 0, 0.52);
    transform: scale(0.95);
    transition:
        transform 0.45s cubic-bezier(0.22, 1, 0.36, 1),
        box-shadow 0.45s ease;
}

.artwork-image.is-playing {
    transform: scale(1);
    box-shadow: 0 28px 64px rgba(0, 0, 0, 0.6);
}

.artwork-fallback {
    display: grid;
    place-items: center;
    background: linear-gradient(
        135deg,
        rgba(255, 255, 255, 0.14),
        rgba(255, 255, 255, 0.04)
    );
    border: 1px solid rgba(255, 255, 255, 0.12);
    color: rgba(255, 255, 255, 0.78);
}

.track-meta {
    width: 100%;
    margin-bottom: 16px;
}

.track-heading {
    text-align: center;
    min-width: 0;
}

.track-title {
    margin: 0;
    font-size: 22px;
    font-weight: 700;
    letter-spacing: -0.015em;
    line-height: 1.3;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.track-sub {
    margin-top: 6px;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    font-size: 14px;
    color: rgba(255, 255, 255, 0.72);
    min-width: 0;
}

.track-sub :deep(.n-button),
.track-sub :deep(button),
.track-sub :deep(a) {
    color: rgba(255, 255, 255, 0.85) !important;
}

.artist-wrap {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.sub-dot {
    opacity: 0.5;
    flex-shrink: 0;
}

.album-btn {
    border: none;
    background: transparent;
    padding: 0;
    margin: 0;
    font: inherit;
    color: rgba(255, 255, 255, 0.82);
    cursor: pointer;
    min-width: 0;
    max-width: 45%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.album-btn:hover {
    color: #ffffff;
    text-decoration: underline;
}

.album-text {
    min-width: 0;
    max-width: 45%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.mobile-badges {
    display: none;
}

.mobile-lyric-preview {
    display: none;
}

.seek-section {
    width: 100%;
    margin-bottom: 16px;
}

.seek-slider,
.volume-slider {
    -webkit-appearance: none;
    appearance: none;
    width: 100%;
    height: 5px;
    border-radius: 999px;
    outline: none;
    cursor: pointer;
    margin: 0;
}

.seek-slider::-webkit-slider-thumb,
.volume-slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: #ffffff;
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.4);
    transition: transform 0.15s ease;
}

.seek-slider:hover::-webkit-slider-thumb,
.volume-slider:hover::-webkit-slider-thumb {
    transform: scale(1.22);
}

.time-row {
    display: flex;
    justify-content: space-between;
    margin-top: 7px;
    font-size: 12px;
    color: rgba(255, 255, 255, 0.65);
    font-variant-numeric: tabular-nums;
}

.transport-controls {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 28px;
    margin-bottom: 16px;
}

.transport-btn {
    position: relative;
    width: 46px;
    height: 46px;
    border-radius: 999px;
    border: none;
    background: rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.9);
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    cursor: pointer;
    transition:
        background-color 0.18s ease,
        transform 0.15s ease;
}

.transport-btn:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.18);
    transform: scale(1.06);
}

.transport-btn:disabled {
    opacity: 0.4;
    cursor: not-allowed;
}

.seek-step {
    font-size: 9px;
    font-weight: 700;
    line-height: 1;
    margin-top: 1px;
    opacity: 0.85;
}

.primary-play-btn {
    width: 66px;
    height: 66px;
    border-radius: 999px;
    border: none;
    background: #ffffff;
    color: #0b0f17;
    display: grid;
    place-items: center;
    cursor: pointer;
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.35);
    transition:
        transform 0.16s ease,
        opacity 0.18s ease;
}

.primary-play-btn:hover:not(:disabled) {
    transform: scale(1.06);
}

.primary-play-btn:disabled {
    opacity: 0.72;
    cursor: wait;
}

.btn-spinner {
    width: 24px;
    height: 24px;
    border-radius: 50%;
    border: 2.5px solid #0b0f17;
    border-top-color: transparent;
    animation: spin 0.75s linear infinite;
}

@keyframes spin {
    to {
        transform: rotate(360deg);
    }
}

.volume-row {
    width: 100%;
    max-width: 240px;
    display: flex;
    align-items: center;
    gap: 10px;
}

.mute-btn {
    border: none;
    background: transparent;
    color: rgba(255, 255, 255, 0.75);
    padding: 4px;
    cursor: pointer;
    display: grid;
    place-items: center;
}

.mute-btn:hover {
    color: #ffffff;
}

.volume-slider {
    flex: 1;
    height: 4px;
}

.playback-feedback {
    margin: 10px 0 0;
    font-size: 12px;
    color: rgba(255, 255, 255, 0.72);
    text-align: center;
    overflow-wrap: anywhere;
}

.playback-feedback.is-error {
    color: #fca5a5;
}

/* 右侧同步歌词舞台 */
.stage-right {
    height: 100%;
    min-height: 0;
    display: flex;
    flex-direction: column;
}

.stage-lyrics-box {
    flex: 1;
    min-height: 0;
}

/* 移动端 / 窄屏自适应 */
@media (max-width: 879px) {
    .player-shell {
        padding: calc(12px + var(--safe-area-top))
            calc(18px + var(--safe-area-right))
            calc(16px + var(--safe-area-bottom))
            calc(18px + var(--safe-area-left));
    }

    .desktop-status-pill {
        display: none;
    }

    .mobile-mode-switch {
        display: inline-flex;
        padding: 3px;
        border-radius: 999px;
        background: rgba(255, 255, 255, 0.1);
        border: 1px solid rgba(255, 255, 255, 0.14);
    }

    .mode-tab {
        padding: 5px 16px;
        border-radius: 999px;
        border: none;
        background: transparent;
        color: rgba(255, 255, 255, 0.72);
        font: inherit;
        font-size: 12px;
        font-weight: 600;
        cursor: pointer;
        transition:
            background-color 0.18s ease,
            color 0.18s ease;
    }

    .mode-tab.is-active {
        background: rgba(255, 255, 255, 0.22);
        color: #ffffff;
    }

    .player-body {
        display: flex;
        flex-direction: column;
        gap: 0;
    }

    .stage-left {
        flex: 1;
        justify-content: space-evenly;
    }

    .stage-right {
        flex: 1;
        width: 100%;
    }

    .is-mobile-hidden {
        display: none !important;
    }

    .artwork-frame {
        width: min(260px, 64vw, 34vh);
        margin-bottom: 12px;
    }

    .track-title {
        font-size: 20px;
    }

    .mobile-badges {
        display: flex;
        justify-content: center;
        gap: 6px;
        margin-top: 8px;
    }

    .mobile-badge {
        padding: 2px 9px;
        border-radius: 999px;
        background: rgba(255, 255, 255, 0.12);
        color: rgba(255, 255, 255, 0.85);
        font-size: 11px;
        font-weight: 600;
    }

    .mobile-badge.is-accent {
        background: var(--color-primary-soft);
        color: var(--color-primary);
    }

    .mobile-lyric-preview {
        width: 100%;
        display: flex;
        flex-direction: column;
        align-items: center;
        gap: 3px;
        padding: 10px 14px;
        margin-bottom: 12px;
        border-radius: 14px;
        border: 1px solid rgba(255, 255, 255, 0.12);
        background: rgba(255, 255, 255, 0.06);
        color: #ffffff;
        font: inherit;
        text-align: center;
        cursor: pointer;
    }

    .preview-current {
        width: 100%;
        font-size: 14px;
        font-weight: 600;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .preview-next,
    .preview-muted {
        width: 100%;
        font-size: 12px;
        color: rgba(255, 255, 255, 0.62);
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .volume-row {
        display: none;
    }

    .mobile-lyrics-mini {
        display: flex;
        align-items: center;
        gap: 12px;
        padding: 10px 12px;
        margin-bottom: 8px;
        border-radius: 16px;
        background: rgba(255, 255, 255, 0.08);
        border: 1px solid rgba(255, 255, 255, 0.12);
        flex-shrink: 0;
    }

    .mini-thumb {
        width: 42px;
        height: 42px;
        border-radius: 10px;
        object-fit: cover;
        flex-shrink: 0;
    }

    .mini-thumb-fallback {
        display: grid;
        place-items: center;
        background: rgba(255, 255, 255, 0.14);
    }

    .mini-thumb-info {
        flex: 1;
        min-width: 0;
    }

    .mini-thumb-title {
        font-size: 14px;
        font-weight: 600;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .mini-thumb-artist {
        font-size: 12px;
        color: rgba(255, 255, 255, 0.68);
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
    }

    .mini-play-pill {
        width: 38px;
        height: 38px;
        border-radius: 999px;
        border: none;
        background: #ffffff;
        color: #0b0f17;
        display: grid;
        place-items: center;
        flex-shrink: 0;
        cursor: pointer;
    }

    .mobile-lyrics-seek {
        display: flex;
        align-items: center;
        gap: 10px;
        padding-top: 8px;
        flex-shrink: 0;
    }

    .mini-time {
        font-size: 11px;
        color: rgba(255, 255, 255, 0.65);
        font-variant-numeric: tabular-nums;
        flex-shrink: 0;
    }
}
</style>
