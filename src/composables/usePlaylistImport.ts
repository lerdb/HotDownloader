import { ref, computed } from 'vue'
import type { PlaylistInfo, SongInfo } from '../types'
import * as musicApi from '../api/musicApi'

/**
 * 歌单导入逻辑封装。
 * 管理输入、歌单信息、歌曲列表、选择状态等，并提供导入和重置方法。
 */
export function usePlaylistImport() {
    // 用户输入的歌单链接或 ID
    const input = ref('')
    // 是否正在导入
    const loading = ref(false)
    // 错误信息
    const errorMsg = ref('')
    // 歌单基本信息
    const playlist = ref<PlaylistInfo | null>(null)
    // 歌曲列表
    const songs = ref<SongInfo[]>([])
    // 选中歌曲的 mid 集合
    const selectedIds = ref<string[]>([])

    // 是否全部选中
    const isAllSelected = computed(() => songs.value.length > 0 && selectedIds.value.length === songs.value.length)
    // 是否部分选中
    const isIndeterminate = computed(() => selectedIds.value.length > 0 && selectedIds.value.length < songs.value.length)

    /**
     * 切换全选状态。
     * @param checked 是否全选
     */
    function toggleAll(checked: boolean) {
        selectedIds.value = checked ? songs.value.map((s) => s.mid) : []
    }

    /**
     * 切换单首歌曲的选中状态。
     * @param songMid 歌曲唯一标识
     * @param selected 是否选中
     */
    function toggleSelect(songMid: string, selected: boolean) {
        if (selected) {
            if (!selectedIds.value.includes(songMid)) {
                selectedIds.value.push(songMid)
            }
        } else {
            selectedIds.value = selectedIds.value.filter((id) => id !== songMid)
        }
    }

    /**
     * 导入歌单。
     * @param platform 平台标识
     * @param term 用户输入的歌单链接或 ID
     */
    async function importPlaylist(platform: string, term: string) {
        const value = term.trim()
        if (!value || loading.value) return

        loading.value = true
        errorMsg.value = ''
        playlist.value = null
        songs.value = []
        selectedIds.value = []

        try {
            const res = await musicApi.fetchPlaylistSongs(platform, value)
            playlist.value = res.playlist
            songs.value = res.songs
        } catch (e: any) {
            errorMsg.value = e?.message || String(e) || '导入歌单失败'
        } finally {
            loading.value = false
        }
    }

    /**
     * 重置所有状态（清空页面）。
     */
    function reset() {
        loading.value = false
        errorMsg.value = ''
        playlist.value = null
        songs.value = []
        selectedIds.value = []
        input.value = ''
    }

    return {
        input,
        loading,
        errorMsg,
        playlist,
        songs,
        selectedIds,
        isAllSelected,
        isIndeterminate,
        toggleAll,
        toggleSelect,
        importPlaylist,
        reset,
    }
}