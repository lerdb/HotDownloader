import { ref, shallowRef } from 'vue'

interface Page<T> {
    items: T[]
    total: number
    hasMore: boolean
}

/** 独立维护分页、失败重试及过期请求，适用于歌手的歌曲和专辑列表。 */
export function usePagedList<T>(key: (item: T) => string) {
    const items = shallowRef<T[]>([])
    const total = ref<number | null>(null)
    const loading = ref(false)
    const hasMore = ref(false)
    const loaded = ref(false)
    const error = ref('')
    let page = 0
    let generation = 0
    let fetcher: ((page: number) => Promise<Page<T>>) | null = null

    function reset() {
        generation++
        items.value = []
        total.value = null
        loading.value = loaded.value = hasMore.value = false
        error.value = ''
        page = 0
        fetcher = null
    }

    async function loadMore() {
        if (!fetcher || loading.value || (loaded.value && !hasMore.value)) return
        const request = generation
        loading.value = true
        error.value = ''
        try {
            const result = await fetcher(page + 1)
            if (request !== generation) return
            const ids = new Set(items.value.map(key))
            items.value = [...items.value, ...result.items.filter(item => {
                const id = key(item)
                if (ids.has(id)) return false
                ids.add(id)
                return true
            })]
            total.value = result.total
            hasMore.value = result.hasMore
            loaded.value = true
            page++
        } catch (e) {
            if (request === generation) error.value = String(e)
        } finally {
            if (request === generation) loading.value = false
        }
    }

    function start(loader: (page: number) => Promise<Page<T>>) {
        reset()
        fetcher = loader
        return loadMore()
    }

    return {
        items,
        total,
        loading,
        hasMore,
        loaded,
        error,
        reset,
        start,
        loadMore
    }
}
