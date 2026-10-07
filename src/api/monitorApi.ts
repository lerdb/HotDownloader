import { webRequest } from './webClient'
import type { SongInfo } from '../types'

export interface MonitorInput {
    name: string
    source: 'public' | 'created' | 'liked'
    playlistId: string
    dirid: string
    quality: string
    intervalMinutes: number
    enabled: boolean
}
export interface Monitor extends MonitorInput {
    id: string
    members: string[]
    lastCheck: number
    nextCheck: number
    lastResult: string
    lastState: string
    counts: Record<string, number>
    initialProgress:
        | { initialized: false }
        | {
              initialized: true
              total: number
              completed: number
              removed: number
              confirmation: number
              failed: number
              active: number
              percent: number
          }
    latestRound: CheckRecord | null
}
export interface CheckRecord {
    startedAt: number
    finishedAt: number
    trigger: 'manual' | 'scheduled' | 'backfill'
    status: 'running' | 'completed' | 'warning' | 'failed' | 'interrupted'
    added: number
    linked: number
    enqueued: number
    failed: number
    message: string
}
export interface LibraryStatus {
    roots: {
        path: string
        mountMarker: string | null
        template: string | null
        artistSeparator: string
    }[]
    fileCount: number
    updatedCount: number
    lastScan: number
    lastSuccess: number
    error: string | null
    unresolvedCount: number
    scanning: boolean
    directories: {
        path: string
        fileCount: number
        observedCount: number | null
        warningCount: number
        state: 'unknown' | 'healthy' | 'warning' | 'unavailable'
        error: string | null
        checkedAt: number
        indexedAt: number
    }[]
}
export interface LocalFile {
    path: string
    metadata: { title: string; artists: string[] }
    filename: { title: string; artists: string[] }
    warning: string | null
    conflict: boolean
}
export interface MonitorSong {
    song: SongInfo
    state: string
    quality: string
    taskId: string | null
    owned: boolean
    path: string | null
    candidates: LocalFile[]
    message: string
    retries: number
    nextRetry: number
}
export const getLibrary = () => webRequest<LibraryStatus>('/api/library')
export const scanLibrary = () =>
    webRequest<LibraryStatus>('/api/library/scan', { method: 'POST' })
export const getMonitors = () =>
    webRequest<{
        monitors: Monitor[]
        running: boolean
        persistenceFailed: boolean
    }>('/api/monitors')
export const getMonitorSongs = (id: string) =>
    webRequest<MonitorSong[]>(`/api/monitors/${encodeURIComponent(id)}/songs`)
export const getMonitorHistory = (id: string) =>
    webRequest<CheckRecord[]>(`/api/monitors/${encodeURIComponent(id)}/history`)
export const saveMonitor = (input: MonitorInput, id?: string) =>
    webRequest<Monitor>(
        id ? `/api/monitors/${encodeURIComponent(id)}` : '/api/monitors',
        { method: id ? 'PATCH' : 'POST', body: JSON.stringify(input) },
    )
export const checkMonitor = (id: string) =>
    webRequest(`/api/monitors/${encodeURIComponent(id)}/check`, {
        method: 'POST',
    })
export const deleteMonitor = (id: string) =>
    webRequest(`/api/monitors/${encodeURIComponent(id)}`, { method: 'DELETE' })
export const decideMonitorSongs = (
    id: string,
    mids: string[],
    action: 'download' | 'ignore',
) =>
    webRequest<{ count: number }>(
        `/api/monitors/${encodeURIComponent(id)}/decisions`,
        {
            method: 'POST',
            body: JSON.stringify({ mids, action }),
        },
    )

export function samePlaylist(a: MonitorInput, b: MonitorInput): boolean {
    const liked = (m: MonitorInput) =>
        m.source === 'liked' ||
        (m.source === 'created' && m.dirid.replace(/^0+/, '') === '201')
    return (
        (liked(a) && liked(b)) ||
        (a.source === b.source &&
            a.source !== 'liked' &&
            !!a.playlistId.trim() &&
            a.playlistId.trim().replace(/^0+/, '') ===
                b.playlistId.trim().replace(/^0+/, '') &&
            (a.source !== 'created' ||
                a.dirid.replace(/^0+/, '') === b.dirid.replace(/^0+/, '')))
    )
}
export const decideSong = (
    monitorId: string,
    mid: string,
    action: 'link' | 'download' | 'ignore' | 'reset',
    path?: string,
) =>
    webRequest(`/api/library/songs/${encodeURIComponent(mid)}`, {
        method: 'POST',
        body: JSON.stringify({ monitorId, action, path }),
    })
