import type { CheckRecord } from '../../api/monitorApi'

export const roundLabel = (status: CheckRecord['status']) =>
    ({
        running: '进行中（统计待完成）',
        completed: '本轮完成',
        warning: '部分派发失败',
        failed: '本轮失败',
        interrupted: '本轮中断（统计不完整）',
    })[status]
export const sourceOptions = [
    { label: 'QQ 公开歌单', value: 'public' },
    { label: '我的 QQ 歌单', value: 'created' },
    { label: '我喜欢', value: 'liked' },
]
export const filterOptions = [
    { label: '全部状态', value: 'all' },
    { label: '待确认', value: 'pending_confirmation' },
    { label: '等待与下载中', value: 'active' },
    { label: '已处理', value: 'done' },
    { label: '失败', value: 'failed' },
]
export const labels: Record<string, string> = {
    pending: '等待匹配',
    ready: '等待入队',
    dispatching: '正在入队',
    queued: '已入队',
    downloading: '下载中',
    paused: '已暂停',
    interrupted: '已中断',
    matched: '已关联',
    downloaded: '已下载',
    ignored: '已忽略',
    pending_confirmation: '待确认',
    no_quality: '无可用音质',
    download_failed: '下载失败',
    credential_invalid: '凭据失效',
}
export const stateLabel = (s: string) => labels[s] || s
export const sourceLabel = (s: string) =>
    sourceOptions.find((o) => o.value === s)?.label
export const canReset = (s: string) =>
    [
        'matched',
        'downloaded',
        'ignored',
        'no_quality',
        'download_failed',
        'credential_invalid',
    ].includes(s)
export const stateType = (
    s: string,
): 'default' | 'success' | 'warning' | 'error' | 'info' =>
    ['matched', 'downloaded'].includes(s)
        ? 'success'
        : s === 'pending_confirmation'
          ? 'warning'
          : ['no_quality', 'download_failed', 'credential_invalid'].includes(s)
            ? 'error'
            : 'default'
export const time = (seconds: number, fallback = '尚未检查') =>
    seconds ? new Date(seconds * 1000).toLocaleString() : fallback

export const monitorErrorMessage = (e: unknown) =>
    e instanceof Error ? e.message : String(e)
