<template>
    <section v-if="player.song" class="playback-bar" aria-label="在线播放器">
        <!-- 顶部细进度指示条 -->
        <div class="mini-progress-track" aria-hidden="true">
            <div
                class="mini-progress-fill"
                :style="{ width: `${progressPercent}%` }"
            />
        </div>

        <div class="mini-player-row">
            <!-- 封面与歌曲信息：点击进入独立全屏播放页面 -->
            <button
                type="button"
                class="track-trigger"
                aria-label="打开全屏播放页面"
                @click="openPlayerPage"
            >
                <div class="mini-cover-wrap">
                    <img
                        v-if="displayCoverUrl"
                        :src="displayCoverUrl"
                        :alt="player.song.title"
                        class="mini-cover"
                    />
                    <div v-else class="mini-cover mini-cover-fallback">
                        <AppIcon name="disc" :size="20" />
                    </div>
                </div>
                <div class="track-info">
                    <div class="track-title" :title="player.song.title">
                        {{ player.song.title }}
                    </div>
                    <div class="track-subtitle">
                        <span class="subtitle-text">{{ subtitleText }}</span>
                        <span v-if="player.source" class="quality-pill">
                            {{ player.source.quality }}
                        </span>
                    </div>
                </div>
            </button>

            <!-- 桌面端内联进度条 -->
            <div class="desktop-seek">
                <span class="time-label">{{
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
                    :aria-valuetext="`${formatTime(player.currentTime)} / ${formatTime(player.duration)}`"
                    @input="player.seek(inputValue($event))"
                />
                <span class="time-label">{{
                    formatTime(player.duration)
                }}</span>
            </div>

            <!-- 控制按钮区 -->
            <div class="mini-actions">
                <label class="volume-control" title="音量">
                    <button
                        type="button"
                        class="icon-btn"
                        :aria-label="player.volume === 0 ? '取消静音' : '静音'"
                        @click="toggleMute"
                    >
                        <AppIcon
                            :name="
                                player.volume === 0 ? 'volume-mute' : 'volume'
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
                </label>

                <button
                    type="button"
                    class="play-circle-btn"
                    :disabled="player.status === 'loading'"
                    :aria-label="playButtonLabel"
                    :title="playButtonLabel"
                    @click="player.toggle()"
                >
                    <span
                        v-if="player.status === 'loading'"
                        class="btn-spinner"
                        aria-hidden="true"
                    />
                    <AppIcon
                        v-else
                        :name="player.status === 'playing' ? 'pause' : 'play'"
                        :size="18"
                    />
                </button>

                <button
                    type="button"
                    class="icon-btn"
                    aria-label="进入播放页"
                    title="打开播放页面与歌词"
                    @click="openPlayerPage"
                >
                    <AppIcon name="expand" :size="18" />
                </button>

                <button
                    type="button"
                    class="icon-btn"
                    aria-label="关闭播放器"
                    title="关闭播放器"
                    @click="player.stop()"
                >
                    <AppIcon name="close" :size="17" />
                </button>
            </div>
        </div>

        <p v-if="player.error" class="player-error" role="alert">
            {{ player.error }}
        </p>
        <p
            v-else-if="player.status === 'loading' || player.buffering"
            class="player-status"
            role="status"
        >
            {{ player.source ? '正在缓冲…' : '正在获取试听链接…' }}
        </p>
    </section>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { fetchCover } from '../api/musicApi'
import { useLyricStore } from '../stores/lyricStore'
import { usePlayerStore } from '../stores/playerStore'
import AppIcon from './common/AppIcon.vue'

const router = useRouter()
const player = usePlayerStore()
const lyricStore = useLyricStore()
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
            // 保持默认图标占位
        }
    },
    { immediate: true },
)

const displayCoverUrl = computed(
    () => player.song?.coverUrl || fetchedCoverUrl.value,
)

const subtitleText = computed(() => {
    if (player.status === 'playing' && lyricStore.activeLine?.text) {
        return lyricStore.activeLine.text
    }
    return player.song?.artist ?? ''
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

function openPlayerPage() {
    void router.push('/player')
}

function sliderBackground(percent: number) {
    const clamped = Math.max(0, Math.min(100, percent))
    return {
        background: `linear-gradient(to right, var(--color-primary) 0%, var(--color-primary) ${clamped}%, var(--border-color-strong) ${clamped}%, var(--border-color-strong) 100%)`,
    }
}

function toggleMute() {
    if (player.volume > 0) {
        lastNonZeroVolume.value = player.volume
        player.setVolume(0)
    } else {
        player.setVolume(lastNonZeroVolume.value || 0.8)
    }
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
.playback-bar {
    position: relative;
    flex-shrink: 0;
    padding: 10px calc(18px + var(--safe-area-right)) 10px
        calc(18px + var(--safe-area-left));
    border-top: 1px solid var(--border-color);
    background: var(--bg-glass-strong);
    backdrop-filter: blur(24px) saturate(165%);
    -webkit-backdrop-filter: blur(24px) saturate(165%);
}

.mini-progress-track {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 2px;
    background: transparent;
    pointer-events: none;
}

.mini-progress-fill {
    height: 100%;
    background: var(--color-primary);
    transition: width 0.2s linear;
}

.mini-player-row {
    display: flex;
    align-items: center;
    gap: 14px;
    min-width: 0;
}

.track-trigger {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 12px;
    border: none;
    background: transparent;
    padding: 2px 4px 2px 0;
    margin: 0;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: pointer;
    border-radius: 10px;
}

.mini-cover-wrap {
    width: 44px;
    height: 44px;
    flex-shrink: 0;
}

.mini-cover {
    width: 44px;
    height: 44px;
    border-radius: 10px;
    object-fit: cover;
    display: block;
    box-shadow: var(--shadow-sm);
}

.mini-cover-fallback {
    display: grid;
    place-items: center;
    background: var(--color-primary-soft);
    color: var(--color-primary);
}

.track-info {
    flex: 1;
    min-width: 0;
}

.track-title {
    font-size: 14px;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--color-text);
}

.track-subtitle {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-top: 2px;
    font-size: 12px;
    color: var(--color-text-secondary);
    min-width: 0;
}

.subtitle-text {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.quality-pill {
    display: inline-flex;
    align-items: center;
    padding: 1px 7px;
    border-radius: 999px;
    background: var(--color-primary-soft);
    color: var(--color-primary);
    font-size: 10px;
    font-weight: 600;
    flex-shrink: 0;
}

.desktop-seek {
    flex: 1.4;
    min-width: 0;
    max-width: 460px;
    display: flex;
    align-items: center;
    gap: 10px;
}

.time-label {
    flex-shrink: 0;
    font-size: 11px;
    color: var(--color-text-secondary);
    font-variant-numeric: tabular-nums;
}

.seek-slider,
.volume-slider {
    -webkit-appearance: none;
    appearance: none;
    height: 4px;
    border-radius: 999px;
    outline: none;
    cursor: pointer;
    margin: 0;
}

.seek-slider {
    flex: 1;
    min-width: 0;
}

.seek-slider::-webkit-slider-thumb,
.volume-slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--color-primary);
    box-shadow: 0 1px 4px rgba(0, 0, 0, 0.3);
    transition: transform 0.15s ease;
}

.seek-slider:hover::-webkit-slider-thumb,
.volume-slider:hover::-webkit-slider-thumb {
    transform: scale(1.2);
}

.mini-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
}

.volume-control {
    display: flex;
    align-items: center;
    gap: 6px;
}

.volume-slider {
    width: 72px;
}

.icon-btn {
    width: 34px;
    height: 34px;
    border-radius: 999px;
    border: none;
    background: transparent;
    color: var(--color-text-secondary);
    display: grid;
    place-items: center;
    cursor: pointer;
    transition:
        background-color 0.18s ease,
        color 0.18s ease;
}

.icon-btn:hover {
    background: var(--bg-surface-subtle);
    color: var(--color-text);
}

.play-circle-btn {
    width: 38px;
    height: 38px;
    border-radius: 999px;
    border: none;
    background: var(--color-primary);
    color: #ffffff;
    display: grid;
    place-items: center;
    cursor: pointer;
    box-shadow: 0 4px 14px var(--color-primary-soft);
    transition:
        transform 0.15s ease,
        background-color 0.18s ease;
}

:root[data-theme='dark'] .play-circle-btn {
    color: #0b1210;
}

.play-circle-btn:hover:not(:disabled) {
    background: var(--color-primary-hover);
    transform: scale(1.05);
}

.play-circle-btn:disabled {
    opacity: 0.7;
    cursor: wait;
}

.btn-spinner {
    width: 16px;
    height: 16px;
    border-radius: 50%;
    border: 2px solid currentColor;
    border-top-color: transparent;
    animation: spin 0.75s linear infinite;
}

@keyframes spin {
    to {
        transform: rotate(360deg);
    }
}

.player-error,
.player-status {
    margin: 6px 0 0;
    font-size: 12px;
    overflow-wrap: anywhere;
}

.player-status {
    color: var(--color-text-secondary);
}

.player-error {
    color: #ef4444;
}

@media (max-width: 920px) {
    .volume-control {
        display: none;
    }
}

@media (max-width: 767px) {
    .playback-bar {
        padding: 8px calc(14px + var(--safe-area-right)) 8px
            calc(14px + var(--safe-area-left));
    }

    .desktop-seek {
        display: none;
    }

    .mini-actions {
        gap: 4px;
    }
}
</style>
