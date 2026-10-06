import { computed, ref, watch, type Ref } from 'vue'
import type { MonitorSong } from '../api/monitorApi'

export function useMonitorSongSelection(
    songs: Ref<MonitorSong[]>,
    selectedId: Ref<string>,
    batchMessage: Ref<string>,
) {
    const selectedMids = ref<string[]>([])
    const filter = ref('all'),
        query = ref(''),
        page = ref(1)
    const filteredSongs = computed(() =>
        songs.value.filter((e) => {
            const category = filter.value
            const matches =
                category === 'all' ||
                e.state === category ||
                (category === 'done' &&
                    ['matched', 'downloaded', 'ignored'].includes(e.state)) ||
                (category === 'active' &&
                    [
                        'pending',
                        'ready',
                        'dispatching',
                        'queued',
                        'downloading',
                        'paused',
                        'interrupted',
                    ].includes(e.state)) ||
                (category === 'failed' &&
                    [
                        'no_quality',
                        'download_failed',
                        'credential_invalid',
                    ].includes(e.state))
            return (
                matches &&
                `${e.song.title} ${e.song.artist}`
                    .toLowerCase()
                    .includes(query.value.trim().toLowerCase())
            )
        }),
    )
    const pageSongs = computed(() =>
        filteredSongs.value.slice((page.value - 1) * 30, page.value * 30),
    )
    const pagePending = computed(() =>
        pageSongs.value.filter((e) => e.state === 'pending_confirmation'),
    )
    const allPagePendingSelected = computed(
        () =>
            pagePending.value.length > 0 &&
            pagePending.value.every((e) =>
                selectedMids.value.includes(e.song.mid),
            ),
    )
    const somePagePendingSelected = computed(() =>
        pagePending.value.some((e) => selectedMids.value.includes(e.song.mid)),
    )
    function selectSong(mid: string, checked: boolean) {
        selectedMids.value = checked
            ? [...new Set([...selectedMids.value, mid])].slice(0, 200)
            : selectedMids.value.filter((id) => id !== mid)
    }
    function selectPagePending(checked: boolean) {
        for (const e of pagePending.value) selectSong(e.song.mid, checked)
    }
    watch(songs, () => {
        selectedMids.value = selectedMids.value.filter((mid) =>
            songs.value.some(
                (e) => e.song.mid === mid && e.state === 'pending_confirmation',
            ),
        )
        page.value = Math.min(
            page.value,
            Math.max(1, Math.ceil(filteredSongs.value.length / 30)),
        )
    })
    watch([filter, query, selectedId], () => {
        page.value = 1
        selectedMids.value = []
        batchMessage.value = ''
    })

    return {
        selectedMids,
        filter,
        query,
        page,
        filteredSongs,
        pageSongs,
        pagePending,
        allPagePendingSelected,
        somePagePendingSelected,
        selectSong,
        selectPagePending,
    }
}
