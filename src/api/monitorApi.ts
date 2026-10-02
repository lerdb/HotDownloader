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
}
export interface LibraryStatus {
    roots: { path: string; template: string | null; artistSeparator: string }[]
    fileCount: number
    updatedCount: number
    lastScan: number
    lastSuccess: number
    error: string | null
    unresolvedCount: number
    scanning: boolean
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
export const saveMonitor = (input: MonitorInput, id?: string) =>
    webRequest<Monitor>(
        id ? `/api/monitors/${encodeURIComponent(id)}` : '/api/monitors',
        { method: id ? 'PATCH' : 'POST', body: JSON.stringify(input) },
    )
export const checkMonitor = (id: string) =>
    webRequest(`/api/monitors/${encodeURIComponent(id)}/check`, {
        method: 'POST',
    })
export const decideSong = (
    mid: string,
    action: 'link' | 'download' | 'ignore' | 'reset',
    path?: string,
) =>
    webRequest(`/api/library/songs/${encodeURIComponent(mid)}`, {
        method: 'POST',
        body: JSON.stringify({ action, path }),
    })
