import { ref } from 'vue'
import type { ArtistInfo } from '../types'
import { searchArtists } from '../api/musicApi'

export function useArtistSearch() {
    const artists = ref<ArtistInfo[]>([])
    const loading = ref(false)
    const loadingMore = ref(false)
    const hasSearched = ref(false)
    const hasMore = ref(false)
    const error = ref('')
    let generation = 0
    let page = 1
    let context = {
        platform: '',
        keyword: ''
    }

    function reset() {
        generation++
        artists.value = []
        loading.value = loadingMore.value = hasSearched.value = hasMore.value = false
        error.value = ''
        page = 1
    }

    async function search(platform: string, keyword: string) {
        reset()
        if (!keyword.trim()) return
        context = {
            platform,
            keyword: keyword.trim()
        }
        hasSearched.value = loading.value = true
        await fetchPage(false)
    }

    async function fetchPage(more: boolean) {
        const request = generation
        const nextPage = more ? page + 1 : 1
        error.value = ''
        try {
            const result = await searchArtists(context.platform, context.keyword, nextPage)
            if (request !== generation) return
            const ids = new Set(artists.value.map(artist => artist.id))
            artists.value = more ? [...artists.value, ...result.artists.filter(artist => !ids.has(artist.id))] : result.artists
            hasMore.value = result.has_more
            page = nextPage
        } catch (e) {
            if (request === generation) error.value = String(e)
        } finally {
            if (request === generation) loading.value = loadingMore.value = false
        }
    }

    async function loadMore() {
        if (loading.value || loadingMore.value || !hasMore.value) return
        loadingMore.value = true
        await fetchPage(true)
    }

    return {
        artists,
        loading,
        loadingMore,
        hasSearched,
        hasMore,
        error,
        reset,
        search,
        loadMore
    }
}
