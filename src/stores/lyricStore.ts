import { defineStore } from 'pinia'
import { computed, ref, watch } from 'vue'
import { getLyricBySongId } from '../api/musicApi'
import type { LyricResponse, SongInfo } from '../types'
import {
    findActiveLyricIndex,
    parseLyricResponse,
    type LyricLine,
} from '../utils/lyrics'
import { usePlayerStore } from './playerStore'

interface CachedLyricEntry {
    instrumental: boolean
    lines: LyricLine[]
    synced: boolean
    hasWordTiming: boolean
}

const MAX_LYRIC_CACHE = 50

export const useLyricStore = defineStore('lyric', () => {
    const player = usePlayerStore()
    const loading = ref(false)
    const error = ref('')
    const instrumental = ref(false)
    const lines = ref<LyricLine[]>([])
    const synced = ref(false)
    const hasWordTiming = ref(false)
    const currentKey = ref('')

    const cache = new Map<string, CachedLyricEntry>()
    let requestGeneration = 0

    function songKey(song: SongInfo): string {
        return `${song.platform}:${song.id}:${song.mid}`
    }

    function applyEntry(entry: CachedLyricEntry) {
        instrumental.value = entry.instrumental
        lines.value = entry.lines
        synced.value = entry.synced
        hasWordTiming.value = entry.hasWordTiming
    }

    function resetState() {
        loading.value = false
        error.value = ''
        instrumental.value = false
        lines.value = []
        synced.value = false
        hasWordTiming.value = false
        currentKey.value = ''
    }

    async function loadForSong(song: SongInfo, force = false) {
        const key = songKey(song)
        currentKey.value = key
        error.value = ''

        if (!force && cache.has(key)) {
            const cached = cache.get(key)!
            // 更新最近使用顺序
            cache.delete(key)
            cache.set(key, cached)
            loading.value = false
            applyEntry(cached)
            return
        }

        const request = ++requestGeneration
        loading.value = true
        instrumental.value = false
        lines.value = []
        synced.value = false
        hasWordTiming.value = false

        try {
            const response: LyricResponse = await getLyricBySongId(
                song.platform,
                song.id,
                song.mid,
            )
            if (request !== requestGeneration) return
            const parsed = parseLyricResponse(response)
            const entry: CachedLyricEntry = {
                instrumental: response.instrumental,
                lines: parsed.lines,
                synced: parsed.synced,
                hasWordTiming: parsed.hasWordTiming,
            }
            cache.set(key, entry)
            while (cache.size > MAX_LYRIC_CACHE) {
                const oldest = cache.keys().next().value
                if (oldest !== undefined) cache.delete(oldest)
            }
            applyEntry(entry)
        } catch (reason) {
            if (request !== requestGeneration) return
            error.value =
                reason instanceof Error ? reason.message : String(reason)
        } finally {
            if (request === requestGeneration) {
                loading.value = false
            }
        }
    }

    watch(
        () => player.song,
        (song) => {
            if (!song) {
                requestGeneration++
                resetState()
                return
            }
            const key = songKey(song)
            if (key === currentKey.value && (loading.value || lines.value.length || instrumental.value)) {
                return
            }
            void loadForSong(song)
        },
        { immediate: true },
    )

    function retry() {
        if (player.song) {
            void loadForSong(player.song, true)
        }
    }

    const activeIndex = computed(() =>
        synced.value ? findActiveLyricIndex(lines.value, player.currentTime) : -1,
    )

    const activeLine = computed<LyricLine | null>(() => {
        if (activeIndex.value < 0) return null
        return lines.value[activeIndex.value] ?? null
    })

    const nextLine = computed<LyricLine | null>(() => {
        if (activeIndex.value < 0) return null
        return lines.value[activeIndex.value + 1] ?? null
    })

    return {
        loading,
        error,
        instrumental,
        lines,
        synced,
        hasWordTiming,
        activeIndex,
        activeLine,
        nextLine,
        retry,
    }
})
