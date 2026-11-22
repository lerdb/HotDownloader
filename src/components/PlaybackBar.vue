<template>
    <section v-if="player.song" class="playback-bar" aria-label="在线播放器">
        <!-- 顶部细进度指示条（SimpMusic 迷你播放器标志性细节） -->
        <div class="mini-progress-track" aria-hidden="true">
            <div
                class="mini-progress-fill"
                :style="{ width: `${progressPercent}%` }"
            />
        </div>

        <div class="mini-player-row">
            <!-- 封面与歌曲信息：点击可展开 SimpMusic 风格正在播放视图 -->
            <button
                type="button"
                class="track-trigger"
                aria-label="展开正在播放页面"
                @click="expanded = true"
            >
                <div class="mini-cover-wrap">
                    <img
                        v-if="player.song.coverUrl"
                        :src="player.song.coverUrl"
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
                        <span>{{ player.song.artist }}</span>
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
                    aria-label="展开播放器"
                    title="正在播放"
                    @click="expanded = true"
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

        <!-- SimpMusic 风格全屏 / 弹窗正在播放视图 -->
        <n-modal
            v-model:show="expanded"
            :trap-focus="false"
            class="now-playing-modal"
        >
            <div class="now-playing-sheet" role="dialog" aria-label="正在播放">
                <div
                    v-if="player.song.coverUrl"
                    class="sheet-backdrop"
                    :style="{
                        backgroundImage: `url(${player.song.coverUrl})`,
                    }"
                    aria-hidden="true"
                />
                <div class="sheet-scrim" aria-hidden="true" />

                <div class="sheet-content">
                    <div class="sheet-topbar">
                        <button
                            type="button"
                            class="sheet-icon-btn"
                            aria-label="收起正在播放"
                            @click="expanded = false"
                        >
                            <AppIcon name="collapse" :size="20" />
                        </button>
                        <span class="sheet-topbar-title">正在试听</span>
                        <button
                            type="button"
                            class="sheet-icon-btn"
                            aria-label="关闭播放器"
                            @click="player.stop()"
                        >
                            <AppIcon name="close" :size="18" />
                        </button>
                    </div>

                    <div class="sheet-artwork-wrap">
                        <img
                            v-if="player.song.coverUrl"
                            :src="player.song.coverUrl"
                            :alt="player.song.title"
                            class="sheet-artwork"
                            :class="{
                                'is-playing': player.status === 'playing',
                            }"
                        />
                        <div
                            v-else
                            class="sheet-artwork sheet-artwork-fallback"
                        >
                            <AppIcon name="disc" :size="72" />
                        </div>
                    </div>

                    <div class="sheet-meta">
                        <h2 class="sheet-title" :title="player.song.title">
                            {{ player.song.title }}
                        </h2>
                        <p class="sheet-artist">
                            {{ player.song.artist }}
                            <template v-if="player.song.album">
                                · {{ player.song.album }}
                            </template>
                        </p>
                        <div class="sheet-badges">
                            <span v-if="player.source" class="sheet-badge">
                                {{ player.source.quality }}
                            </span>
                            <span class="sheet-badge sheet-badge-muted">
                                {{ platformLabel }}
                            </span>
                        </div>
                    </div>

                    <div class="sheet-seek-area">
                        <input
                            class="seek-slider sheet-seek-slider"
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
                        <div class="sheet-times">
                            <span>{{ formatTime(player.currentTime) }}</span>
                            <span>{{ formatTime(player.duration) }}</span>
                        </div>
                    </div>

                    <div class="sheet-controls">
                        <button
                            type="button"
                            class="sheet-secondary-btn"
                            title="静音切换"
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
                                :size="20"
                            />
                        </button>

                        <button
                            type="button"
                            class="sheet-play-btn"
                            :disabled="player.status === 'loading'"
                            :aria-label="playButtonLabel"
                            @click="player.toggle()"
                        >
                            <span
                                v-if="player.status === 'loading'"
                                class="btn-spinner btn-spinner-lg"
                                aria-hidden="true"
                            />
                            <AppIcon
                                v-else
                                :name="
                                    player.status === 'playing'
                                        ? 'pause'
                                        : 'play'
                                "
                                :size="28"
                            />
                        </button>

                        <button
                            type="button"
                            class="sheet-secondary-btn"
                            title="下载此歌曲"
                            aria-label="下载当前歌曲"
                            @click="downloadCurrent"
                        >
                            <AppIcon name="download" :size="20" />
                        </button>
                    </div>

                    <div class="sheet-volume-row">
                        <AppIcon name="volume" :size="15" />
                        <input
                            class="volume-slider sheet-volume-slider"
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

                    <p v-if="player.error" class="sheet-status is-error">
                        {{ player.error }}
                    </p>
                    <p
                        v-else-if="
                            player.status === 'loading' || player.buffering
                        "
                        class="sheet-status"
                    >
                        {{ player.source ? '正在缓冲…' : '正在获取试听链接…' }}
                    </p>
                </div>
            </div>
        </n-modal>
    </section>
</template>

<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { NModal } from 'naive-ui'
import { usePlayerStore } from '../stores/playerStore'
import { useDownloadActions } from '../composables/useDownloadActions'
import { PLATFORMS } from '../config/platforms'
import AppIcon from './common/AppIcon.vue'

const player = usePlayerStore()
const { downloadSingle } = useDownloadActions()
const expanded = ref(false)
const lastNonZeroVolume = ref(0.8)

watch(
    () => player.song,
    (song) => {
        if (!song) expanded.value = false
    },
)

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

const platformLabel = computed(() => {
    const key = player.song?.platform
    if (!key) return ''
    return PLATFORMS.find((p) => p.key === key)?.label ?? key
})

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

function downloadCurrent() {
    if (!player.song) return
    void downloadSingle(player.song)
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

:root[data-theme='dark'] .play-circle-btn,
:root[data-theme='dark'] .sheet-play-btn {
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

.btn-spinner-lg {
    width: 24px;
    height: 24px;
    border-width: 2.5px;
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

/* ---------- SimpMusic 正在播放弹窗 ---------- */
:global(.now-playing-modal) {
    width: min(440px, calc(100vw - 24px)) !important;
    max-width: 440px !important;
    background: transparent !important;
    box-shadow: none !important;
}

.now-playing-sheet {
    position: relative;
    overflow: hidden;
    border-radius: 26px;
    background: var(--bg-elevated);
    border: 1px solid var(--border-color);
    box-shadow: var(--shadow-lg);
    color: var(--color-text);
}

.sheet-backdrop {
    position: absolute;
    inset: -32px;
    background-size: cover;
    background-position: center;
    filter: blur(42px) saturate(150%);
    opacity: 0.34;
    transform: scale(1.1);
}

.sheet-scrim {
    position: absolute;
    inset: 0;
    background: linear-gradient(
        180deg,
        rgba(13, 15, 20, 0.22) 0%,
        var(--bg-glass-strong) 82%
    );
}

.sheet-content {
    position: relative;
    z-index: 1;
    padding: 20px 24px 26px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 18px;
}

.sheet-topbar {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
}

.sheet-topbar-title {
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--color-text-secondary);
}

.sheet-icon-btn {
    width: 36px;
    height: 36px;
    border-radius: 999px;
    border: none;
    background: var(--bg-surface-subtle);
    color: var(--color-text);
    display: grid;
    place-items: center;
    cursor: pointer;
}

.sheet-artwork-wrap {
    width: min(240px, 64vw);
    aspect-ratio: 1 / 1;
    margin: 4px 0;
}

.sheet-artwork {
    width: 100%;
    height: 100%;
    border-radius: 20px;
    object-fit: cover;
    display: block;
    box-shadow: 0 18px 40px rgba(0, 0, 0, 0.32);
    transition: transform 0.3s ease;
}

.sheet-artwork.is-playing {
    transform: scale(1.02);
}

.sheet-artwork-fallback {
    display: grid;
    place-items: center;
    background: var(--color-primary-soft);
    color: var(--color-primary);
}

.sheet-meta {
    width: 100%;
    text-align: center;
    min-width: 0;
}

.sheet-title {
    font-size: 20px;
    font-weight: 700;
    line-height: 1.3;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    margin: 0;
}

.sheet-artist {
    margin: 6px 0 0;
    font-size: 14px;
    color: var(--color-text-secondary);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

.sheet-badges {
    margin-top: 10px;
    display: flex;
    justify-content: center;
    gap: 8px;
}

.sheet-badge {
    padding: 3px 10px;
    border-radius: 999px;
    font-size: 11px;
    font-weight: 600;
    background: var(--color-primary-soft);
    color: var(--color-primary);
}

.sheet-badge-muted {
    background: var(--bg-surface-subtle);
    color: var(--color-text-secondary);
}

.sheet-seek-area {
    width: 100%;
}

.sheet-seek-slider {
    width: 100%;
}

.sheet-times {
    display: flex;
    justify-content: space-between;
    margin-top: 6px;
    font-size: 12px;
    color: var(--color-text-secondary);
    font-variant-numeric: tabular-nums;
}

.sheet-controls {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 24px;
}

.sheet-secondary-btn {
    width: 44px;
    height: 44px;
    border-radius: 999px;
    border: 1px solid var(--border-color);
    background: var(--bg-surface-subtle);
    color: var(--color-text);
    display: grid;
    place-items: center;
    cursor: pointer;
    transition: transform 0.15s ease;
}

.sheet-secondary-btn:hover {
    transform: scale(1.06);
}

.sheet-play-btn {
    width: 62px;
    height: 62px;
    border-radius: 999px;
    border: none;
    background: var(--color-primary);
    color: #ffffff;
    display: grid;
    place-items: center;
    cursor: pointer;
    box-shadow: 0 10px 24px var(--color-primary-soft);
    transition:
        transform 0.15s ease,
        background-color 0.18s ease;
}

.sheet-play-btn:hover:not(:disabled) {
    background: var(--color-primary-hover);
    transform: scale(1.04);
}

.sheet-volume-row {
    width: 100%;
    max-width: 240px;
    display: flex;
    align-items: center;
    gap: 10px;
    color: var(--color-text-secondary);
}

.sheet-volume-slider {
    flex: 1;
    width: auto;
}

.sheet-status {
    margin: 0;
    font-size: 12px;
    color: var(--color-text-secondary);
    text-align: center;
}

.sheet-status.is-error {
    color: #ef4444;
}
</style>
