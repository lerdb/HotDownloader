<template>
    <div class="lyric-stage">
        <div v-if="lyricStore.loading" class="lyric-state">
            <div class="lyric-spinner" aria-hidden="true" />
            <span>正在加载同步歌词…</span>
        </div>

        <div v-else-if="lyricStore.error" class="lyric-state">
            <p class="state-title">歌词加载失败</p>
            <p class="state-desc">{{ lyricStore.error }}</p>
            <button
                type="button"
                class="state-retry-btn"
                @click="lyricStore.retry()"
            >
                重试加载
            </button>
        </div>

        <div v-else-if="lyricStore.instrumental" class="lyric-state">
            <div class="state-icon">
                <AppIcon name="music" :size="32" />
            </div>
            <p class="state-title">纯音乐，请欣赏</p>
            <p class="state-desc">当前曲目暂无演唱歌词</p>
        </div>

        <div v-else-if="!lyricStore.lines.length" class="lyric-state">
            <div class="state-icon">
                <AppIcon name="lyric" :size="32" />
            </div>
            <p class="state-title">暂无歌词</p>
            <p class="state-desc">暂未获取到该歌曲的歌词信息</p>
        </div>

        <div v-else class="lyric-scroll-wrap">
            <div
                ref="scrollRef"
                class="lyric-scroll"
                @wheel.passive="pauseAutoScroll"
                @touchstart.passive="pauseAutoScroll"
                @pointerdown.passive="pauseAutoScroll"
            >
                <div class="lyric-spacer" aria-hidden="true" />

                <button
                    v-for="(line, index) in lyricStore.lines"
                    :key="`${index}-${line.time}`"
                    :ref="(el) => setLineRef(index, el)"
                    type="button"
                    class="lyric-line"
                    :class="{
                        'is-active':
                            lyricStore.synced &&
                            index === lyricStore.activeIndex,
                        'is-passed':
                            lyricStore.synced &&
                            index < lyricStore.activeIndex,
                        'is-unsynced': !lyricStore.synced,
                        'is-clickable': lyricStore.synced,
                    }"
                    :disabled="!lyricStore.synced"
                    @click="handleLineClick(line.time)"
                >
                    <span
                        v-if="
                            line.words?.length &&
                            lyricStore.synced &&
                            index === lyricStore.activeIndex
                        "
                        class="lyric-main lyric-words"
                    >
                        <span
                            v-for="(word, wIdx) in line.words"
                            :key="wIdx"
                            class="lyric-word"
                            :style="wordStyle(word.startTime, word.endTime)"
                            >{{ word.text }}</span
                        >
                    </span>
                    <span v-else class="lyric-main">{{ line.text }}</span>

                    <span v-if="line.translation" class="lyric-translation">
                        {{ line.translation }}
                    </span>
                </button>

                <div class="lyric-spacer" aria-hidden="true" />
            </div>

            <transition name="fade-up">
                <button
                    v-if="userScrolling && lyricStore.synced && lyricStore.activeIndex >= 0"
                    type="button"
                    class="sync-back-pill"
                    @click="resumeAutoScroll"
                >
                    回到当前歌词
                </button>
            </transition>
        </div>
    </div>
</template>

<script setup lang="ts">
import { nextTick, onBeforeUnmount, ref, watch, type ComponentPublicInstance } from 'vue'
import { useLyricStore } from '../../stores/lyricStore'
import { usePlayerStore } from '../../stores/playerStore'
import AppIcon from '../common/AppIcon.vue'

const lyricStore = useLyricStore()
const player = usePlayerStore()

const scrollRef = ref<HTMLElement | null>(null)
const lineElements = new Map<number, HTMLElement>()
const userScrolling = ref(false)
let resumeTimer: ReturnType<typeof setTimeout> | null = null

function setLineRef(
    index: number,
    el: Element | ComponentPublicInstance | null,
) {
    if (el instanceof HTMLElement) {
        lineElements.set(index, el)
    } else {
        lineElements.delete(index)
    }
}

function clearResumeTimer() {
    if (resumeTimer) {
        clearTimeout(resumeTimer)
        resumeTimer = null
    }
}

function pauseAutoScroll() {
    if (!lyricStore.synced) return
    userScrolling.value = true
    clearResumeTimer()
    resumeTimer = setTimeout(() => {
        userScrolling.value = false
        scrollToActive('smooth')
    }, 3500)
}

function resumeAutoScroll() {
    clearResumeTimer()
    userScrolling.value = false
    scrollToActive('smooth')
}

function scrollToActive(behavior: ScrollBehavior = 'smooth') {
    const container = scrollRef.value
    const index = lyricStore.activeIndex
    if (!container || index < 0) return
    const target = lineElements.get(index)
    if (!target) return

    const targetTop =
        target.offsetTop - container.clientHeight / 2 + target.clientHeight / 2
    container.scrollTo({
        top: Math.max(0, targetTop),
        behavior,
    })
}

function handleLineClick(time: number) {
    if (!lyricStore.synced) return
    player.seek(time)
    resumeAutoScroll()
}

function wordStyle(startTime: number, endTime: number) {
    const now = player.currentTime
    if (now >= endTime) {
        return { color: '#ffffff' }
    }
    if (now <= startTime) {
        return { color: 'rgba(255, 255, 255, 0.42)' }
    }
    const span = Math.max(0.05, endTime - startTime)
    const pct = Math.min(100, Math.max(0, ((now - startTime) / span) * 100))
    return {
        backgroundImage: `linear-gradient(90deg, #ffffff 0%, #ffffff ${pct.toFixed(1)}%, rgba(255, 255, 255, 0.42) ${pct.toFixed(1)}%)`,
        WebkitBackgroundClip: 'text',
        backgroundClip: 'text',
        color: 'transparent',
    }
}

watch(
    () => lyricStore.activeIndex,
    async () => {
        if (userScrolling.value) return
        await nextTick()
        scrollToActive('smooth')
    },
)

watch(
    () => lyricStore.lines,
    async () => {
        lineElements.clear()
        clearResumeTimer()
        userScrolling.value = false
        await nextTick()
        if (lyricStore.activeIndex >= 0) {
            scrollToActive('auto')
        } else if (scrollRef.value) {
            scrollRef.value.scrollTop = 0
        }
    },
)

onBeforeUnmount(() => {
    clearResumeTimer()
    lineElements.clear()
})
</script>

<style scoped>
.lyric-stage {
    position: relative;
    width: 100%;
    height: 100%;
    min-height: 0;
    display: flex;
    flex-direction: column;
}

.lyric-state {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 10px;
    padding: 32px 20px;
    text-align: center;
    color: rgba(255, 255, 255, 0.72);
}

.state-icon {
    width: 60px;
    height: 60px;
    border-radius: 18px;
    background: rgba(255, 255, 255, 0.08);
    display: grid;
    place-items: center;
    color: rgba(255, 255, 255, 0.86);
    margin-bottom: 4px;
}

.state-title {
    margin: 0;
    font-size: 16px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.92);
}

.state-desc {
    margin: 0;
    font-size: 13px;
    color: rgba(255, 255, 255, 0.56);
    max-width: 300px;
    overflow-wrap: anywhere;
}

.state-retry-btn {
    margin-top: 6px;
    padding: 8px 18px;
    border-radius: 999px;
    border: 1px solid rgba(255, 255, 255, 0.22);
    background: rgba(255, 255, 255, 0.12);
    color: #ffffff;
    font: inherit;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
}

.state-retry-btn:hover {
    background: rgba(255, 255, 255, 0.2);
}

.lyric-spinner {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    border: 2.5px solid rgba(255, 255, 255, 0.22);
    border-top-color: #ffffff;
    animation: spin 0.8s linear infinite;
}

@keyframes spin {
    to {
        transform: rotate(360deg);
    }
}

.lyric-scroll-wrap {
    position: relative;
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
}

.lyric-scroll {
    position: relative;
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 0 12px;
    scrollbar-width: none;
    -webkit-mask-image: linear-gradient(
        to bottom,
        transparent 0%,
        #000 12%,
        #000 88%,
        transparent 100%
    );
    mask-image: linear-gradient(
        to bottom,
        transparent 0%,
        #000 12%,
        #000 88%,
        transparent 100%
    );
}

.lyric-scroll::-webkit-scrollbar {
    display: none;
}

.lyric-spacer {
    height: 36%;
}

.lyric-line {
    width: 100%;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 5px;
    padding: 12px 14px;
    border: none;
    border-radius: 14px;
    background: transparent;
    color: rgba(255, 255, 255, 0.42);
    font: inherit;
    text-align: left;
    transform-origin: left center;
    transition:
        color 0.28s ease,
        transform 0.32s cubic-bezier(0.22, 1, 0.36, 1),
        background-color 0.2s ease,
        opacity 0.28s ease;
}

.lyric-line.is-clickable {
    cursor: pointer;
}

.lyric-line.is-clickable:hover {
    background: rgba(255, 255, 255, 0.08);
    color: rgba(255, 255, 255, 0.78);
}

.lyric-line.is-passed {
    color: rgba(255, 255, 255, 0.36);
}

.lyric-line.is-active {
    color: #ffffff;
    transform: scale(1.035);
    text-shadow: 0 2px 18px rgba(255, 255, 255, 0.22);
}

.lyric-line.is-unsynced {
    color: rgba(255, 255, 255, 0.86);
    cursor: default;
}

.lyric-main {
    font-size: 22px;
    font-weight: 700;
    line-height: 1.42;
    letter-spacing: -0.01em;
    overflow-wrap: anywhere;
}

.lyric-line:not(.is-active):not(.is-unsynced) .lyric-main {
    font-size: 19px;
    font-weight: 600;
}

.lyric-words {
    display: inline;
}

.lyric-word {
    white-space: pre-wrap;
}

.lyric-translation {
    font-size: 14px;
    font-weight: 500;
    line-height: 1.45;
    color: rgba(255, 255, 255, 0.62);
    overflow-wrap: anywhere;
}

.lyric-line.is-active .lyric-translation {
    color: rgba(255, 255, 255, 0.84);
}

.sync-back-pill {
    position: absolute;
    left: 50%;
    bottom: 18px;
    transform: translateX(-50%);
    padding: 7px 16px;
    border-radius: 999px;
    border: 1px solid rgba(255, 255, 255, 0.22);
    background: rgba(20, 24, 34, 0.78);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    color: #ffffff;
    font: inherit;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
}

.fade-up-enter-active,
.fade-up-leave-active {
    transition:
        opacity 0.2s ease,
        transform 0.2s ease;
}

.fade-up-enter-from,
.fade-up-leave-to {
    opacity: 0;
    transform: translate(-50%, 8px);
}

@media (max-width: 879px) {
    .lyric-line {
        align-items: center;
        text-align: center;
        transform-origin: center center;
        padding: 11px 10px;
    }

    .lyric-main {
        font-size: 20px;
    }

    .lyric-line:not(.is-active):not(.is-unsynced) .lyric-main {
        font-size: 17px;
    }
}
</style>
