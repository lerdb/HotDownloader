import { ref } from 'vue'
import type { AlbumInfo } from '../types'
import { searchAlbums } from '../api/musicApi'

export function useAlbumSearch() {
    const albums = ref<AlbumInfo[]>([])
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
        albums.value = []
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
            const result = await searchAlbums(context.platform, context.keyword, nextPage)
            if (request !== generation) return
            const ids = new Set(albums.value.map(album => album.id))
            albums.value = more ? [...albums.value, ...result.albums.filter(album => !ids.has(album.id))] : result.albums
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
        albums,
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
