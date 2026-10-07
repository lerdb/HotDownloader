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
    const pageSongs = computed(() => songs.value)
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
        // 仅移除当前页已失效的选择，其他页的选择交由提交时服务端校验。
        const visible = new Map(songs.value.map((e) => [e.song.mid, e.state]))
        selectedMids.value = selectedMids.value.filter(
            (mid) =>
                !visible.has(mid) ||
                visible.get(mid) === 'pending_confirmation',
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
        pageSongs,
        pagePending,
        allPagePendingSelected,
        somePagePendingSelected,
        selectSong,
        selectPagePending,
    }
}
