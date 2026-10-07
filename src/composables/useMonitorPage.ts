import {
    computed,
    onActivated,
    onDeactivated,
    onUnmounted,
    ref,
    watch,
} from 'vue'
import { useRoute, useRouter } from 'vue-router'
import * as api from '../api/monitorApi'
import { fetchCreatedPlaylists } from '../api/musicApi'
import type { PlaylistSearchItem } from '../types'
import { useSettingsStore } from '../stores/settingsStore'

import { useMonitorHistory } from './useMonitorHistory'
import { useMonitorSongSelection } from './useMonitorSongSelection'
import { monitorErrorMessage as message } from '../components/monitor/presentation'

export function useMonitorPage() {
    const router = useRouter()
    const route = useRoute()
    const settings = useSettingsStore()
    const library = ref<api.LibraryStatus>()
    const monitors = ref<api.Monitor[]>([])
    const songs = ref<api.MonitorSong[]>([])
    const totalSongs = ref(0)
    const batchMessage = ref('')
    const {
        showHistory,
        historyLoading,
        historyError,
        historyName,
        history,
        historyPage,
        openHistory,
        loadHistory,
    } = useMonitorHistory()
    const selectedId = ref('')
    const selectedName = computed(
        () =>
            monitors.value.find((m) => m.id === selectedId.value)?.name ||
            '歌单',
    )
    const error = ref(''),
        editorError = ref(''),
        linkError = ref('')
    const busy = ref(false),
        scanning = ref(false),
        running = ref(false),
        persistenceFailed = ref(false)
    const showEditor = ref(false),
        editingId = ref(''),
        showLink = ref(false)
    const linking = ref<api.MonitorSong>(),
        linkPath = ref('')
    const form = ref<api.MonitorInput>({
        name: '',
        source: 'public',
        playlistId: '',
        dirid: '',
        quality: '',
        intervalMinutes: 60,
        enabled: true,
    })
    const duplicateMonitor = computed(() =>
        monitors.value.find((m) => api.samePlaylist(m, form.value)),
    )
    const {
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
    } = useMonitorSongSelection(songs, selectedId, batchMessage)
    let timer: ReturnType<typeof setTimeout> | undefined
    let active = false
    let refreshPromise: Promise<void> | undefined
    let pollGeneration = 0
    function refresh(): Promise<void> {
        if (!refreshPromise)
            refreshPromise = loadSnapshot().finally(() => {
                refreshPromise = undefined
            })
        return refreshPromise
    }
    async function loadSnapshot() {
        try {
            const [lib, result] = await Promise.all([
                api.getLibrary(),
                api.getMonitors(),
            ])
            library.value = lib
            monitors.value = result.monitors
            running.value = result.running
            persistenceFailed.value = result.persistenceFailed
            if (showHistory.value && !historyLoading.value) await loadHistory()
            error.value = ''
            const id = selectedId.value
            if (id && !monitors.value.some((m) => m.id === id)) {
                selectedId.value = ''
                songs.value = []
            } else if (id) {
                await loadSongs()
            }
        } catch (e) {
            error.value = message(e)
        }
    }
    async function poll(generation: number) {
        await refresh()
        if (active && generation === pollGeneration)
            timer = setTimeout(() => void poll(generation), 15000)
    }
    let songRequest = 0
    let queryTimer: ReturnType<typeof setTimeout> | undefined
    async function loadSongs() {
        const id = selectedId.value
        if (!id) return
        const request = ++songRequest
        const requestedPage = page.value,
            requestedFilter = filter.value,
            requestedQuery = query.value
        try {
            const result = await api.getMonitorSongs(
                id,
                requestedPage,
                requestedFilter,
                requestedQuery,
            )
            if (
                request !== songRequest ||
                id !== selectedId.value ||
                requestedPage !== page.value ||
                requestedFilter !== filter.value ||
                requestedQuery !== query.value
            )
                return
            songs.value = result.items
            totalSongs.value = result.total
            page.value = result.page
        } catch (e) {
            if (request === songRequest && id === selectedId.value)
                error.value = message(e)
        }
    }
    watch([page, filter, query], (_value, previous) => {
        clearTimeout(queryTimer)
        if (query.value !== previous[2])
            queryTimer = setTimeout(() => void loadSongs(), 300)
        else void loadSongs()
    })
    function stop() {
        active = false
        pollGeneration++
        clearTimeout(timer)
        clearTimeout(queryTimer)
        songRequest++
    }
    onActivated(() => {
        if (!active) {
            active = true
            void poll(++pollGeneration)
        }
        consumeAddRoute()
    })
    onDeactivated(stop)
    onUnmounted(stop)
    async function selectMonitor(id: string) {
        selectedId.value = id
        songRequest++
        totalSongs.value = 0
        songs.value = []
        await refreshPromise
        await refresh()
    }
    async function perform(action: () => Promise<unknown>) {
        if (busy.value) return
        busy.value = true
        try {
            await refreshPromise
            await action()
            await refresh()
        } catch (e) {
            error.value = message(e)
        } finally {
            busy.value = false
        }
    }
    async function scan() {
        scanning.value = true
        try {
            await api.scanLibrary()
            await refresh()
        } catch (e) {
            error.value = message(e)
        } finally {
            scanning.value = false
        }
    }
    async function check(m: api.Monitor) {
        await perform(() => api.checkMonitor(m.id))
    }
    async function remove(m: api.Monitor) {
        await perform(async () => {
            await api.deleteMonitor(m.id)
            if (selectedId.value === m.id) {
                selectedId.value = ''
                songs.value = []
            }
        })
    }
    async function batchDecide(action: 'download' | 'ignore') {
        const id = selectedId.value
        const mids = [...selectedMids.value]
        if (!id || !mids.length) return
        batchMessage.value = ''
        await perform(async () => {
            const result = await api.decideMonitorSongs(id, mids, action)
            selectedMids.value = []
            batchMessage.value = `已${action === 'download' ? '请求下载' : '忽略'} ${result.count} 首歌曲`
        })
    }
    function inputOf(m: api.Monitor): api.MonitorInput {
        return {
            name: m.name,
            source: m.source,
            playlistId: m.playlistId,
            dirid: m.dirid,
            quality: m.quality,
            intervalMinutes: m.intervalMinutes,
            enabled: m.enabled,
        }
    }
    async function toggle(m: api.Monitor, enabled: boolean) {
        await perform(() => api.saveMonitor({ ...inputOf(m), enabled }, m.id))
    }
    function edit(m?: api.Monitor) {
        editingId.value = m?.id || ''
        editorError.value = ''
        form.value = m
            ? inputOf(m)
            : {
                  name: '',
                  source: 'public',
                  playlistId: '',
                  dirid: '',
                  quality:
                      settings.settings.defaultQuality === 'ask'
                          ? ''
                          : settings.settings.defaultQuality,
                  intervalMinutes: 60,
                  enabled: true,
              }
        showEditor.value = true
    }
    function consumeAddRoute() {
        if (route.path !== '/playlist/monitors' || route.query.add !== '1')
            return
        const q = route.query
        edit()
        if (
            q.source === 'public' ||
            q.source === 'created' ||
            q.source === 'liked'
        )
            form.value.source = q.source
        form.value.name = typeof q.name === 'string' ? q.name : ''
        form.value.playlistId =
            form.value.source !== 'liked' && typeof q.playlistId === 'string'
                ? q.playlistId
                : ''
        form.value.dirid =
            form.value.source === 'created' && typeof q.dirid === 'string'
                ? q.dirid
                : ''
        void router.replace({ path: '/playlist/monitors' })
    }
    watch(() => route.fullPath, consumeAddRoute)
    async function save() {
        if (busy.value) return
        if (!form.value.quality || !form.value.intervalMinutes) {
            editorError.value = '请选择自动下载音质并填写检查间隔'
            return
        }
        busy.value = true
        try {
            await refreshPromise
            await api.saveMonitor(form.value, editingId.value || undefined)
            showEditor.value = false
            await refresh()
        } catch (e) {
            editorError.value = message(e)
        } finally {
            busy.value = false
        }
    }
    async function decide(
        e: api.MonitorSong,
        action: 'download' | 'ignore' | 'reset' | 'refresh' | 'retry',
    ) {
        const id = selectedId.value
        await perform(() => api.decideSong(id, e.song.mid, action))
    }
    function openLink(e: api.MonitorSong) {
        linking.value = e
        linkPath.value = ''
        linkError.value = ''
        showLink.value = true
    }
    async function confirmLink() {
        if (!linking.value) return
        busy.value = true
        try {
            await api.decideSong(
                selectedId.value,
                linking.value.song.mid,
                'link',
                linkPath.value,
            )
            showLink.value = false
            await refresh()
        } catch (e) {
            linkError.value = message(e)
        } finally {
            busy.value = false
        }
    }
    const playlists = ref<PlaylistSearchItem[]>([]),
        loadingPlaylists = ref(false)
    const playlistOptions = computed(() =>
        playlists.value.map((p, i) => ({ label: p.name, value: i })),
    )
    async function loadPlaylists() {
        loadingPlaylists.value = true
        try {
            playlists.value = (await fetchCreatedPlaylists()).playlists
        } catch (e) {
            editorError.value = message(e)
        } finally {
            loadingPlaylists.value = false
        }
    }
    function choosePlaylist(i: number) {
        const p = playlists.value[i]
        if (p) {
            form.value.playlistId = p.id
            form.value.dirid = p.dirid || ''
            form.value.name = p.name
        }
    }

    return {
        library,
        monitors,
        songs,
        totalSongs,
        selectedMids,
        batchMessage,
        showHistory,
        historyLoading,
        historyError,
        historyName,
        history,
        historyPage,
        openHistory,
        loadHistory,
        selectedId,
        selectedName,
        error,
        editorError,
        linkError,
        busy,
        scanning,
        running,
        persistenceFailed,
        showEditor,
        editingId,
        showLink,
        linking,
        linkPath,
        filter,
        query,
        page,
        form,
        duplicateMonitor,
        pageSongs,
        pagePending,
        allPagePendingSelected,
        somePagePendingSelected,
        selectSong,
        selectPagePending,
        selectMonitor,
        scan,
        check,
        remove,
        batchDecide,
        toggle,
        edit,
        save,
        decide,
        openLink,
        confirmLink,
        loadingPlaylists,
        playlistOptions,
        loadPlaylists,
        choosePlaylist,
    }
}
