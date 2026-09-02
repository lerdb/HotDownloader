import { h, ref } from 'vue'
import { useRouter } from 'vue-router'
import { useDialog, useNotification, NButton } from 'naive-ui'
import type { Quality, SongInfo, QualityItem } from '../types'
import { QUALITY_DOWNGRADE_ORDER } from '../types'
import { useSettingsStore } from '../stores/settingsStore'
import { useTaskStore } from '../stores/taskStore'
import QualitySelector from '../components/search/QualitySelector.vue'
import * as musicApi from '../api/musicApi'

// 下载逻辑
export function useDownloadActions() {
    const dialog = useDialog()
    const router = useRouter()
    const notification = useNotification()
    const settingsStore = useSettingsStore()
    const taskStore = useTaskStore()

    function generateTaskId(): string {
        return Date.now().toString(36) + Math.random().toString(36).substring(2)
    }

    /** 弹出品质选择弹窗，返回选中的品质标签 */
    function askQuality(qualities: QualityItem[]): Promise<string> {
        return new Promise((resolve, reject) => {
            const compRef = ref<InstanceType<typeof QualitySelector>>()

            const d = dialog.create({
                title: '选择下载音质',
                content: () =>
                    h(QualitySelector, { qualities, ref: compRef }),
                positiveText: '确定',
                negativeText: '取消',
                onPositiveClick: () => {
                    const val = compRef.value?.selected
                    if (val) {
                        resolve(val)
                    } else {
                        reject(new Error('未选择品质'))
                    }
                    d.destroy()
                },
                onNegativeClick: () => {
                    reject(new Error('用户取消'))
                    d.destroy()
                },
                onClose: () => {
                    reject(new Error('用户取消'))
                    d.destroy()
                },
            })
        })
    }

    /** 弹出重复文件处理选择框，返回用户选择 */
    function askDuplicateAction(songTitle: string): Promise<'overwrite' | 'rename' | 'cancel'> {
        return new Promise((resolve) => {
            const d = dialog.create({
                title: '文件已存在',
                content: `歌曲“${songTitle}”在下载目录中已存在同名文件，请选择处理方式：`,
                action: () => [
                    h(
                        NButton,
                        {
                            size: 'small',
                            onClick: () => {
                                resolve('overwrite')
                                d.destroy()
                            },
                        },
                        { default: () => '覆盖' }
                    ),
                    h(
                        NButton,
                        {
                            size: 'small',
                            type: 'primary',
                            onClick: () => {
                                resolve('rename')
                                d.destroy()
                            },
                        },
                        { default: () => '保留两份' }
                    ),
                    h(
                        NButton,
                        {
                            size: 'small',
                            type: 'error',
                            onClick: () => {
                                resolve('cancel')
                                d.destroy()
                            },
                        },
                        { default: () => '取消' }
                    ),
                ],
            });
        });
    }

    /**
     * 根据期望品质和歌曲可用品质列表，返回实际可用的品质项（含 filename）
     * 若无法满足且开启自动降级，则按降级顺序选择第一个可用品质
     * 若仍无可用品质，返回 null
     */
    function resolveQualityForSong(
        song: SongInfo,
        desiredQuality: Quality
    ): QualityItem | null {
        const direct = song.qualities.find((q) => q.quality === desiredQuality)
        if (direct) return direct

        if (settingsStore.settings.autoDowngrade) {
            for (const fallback of QUALITY_DOWNGRADE_ORDER) {
                const found = song.qualities.find((q) => q.quality === fallback)
                if (found) return found
            }
        }
        return null
    }

    /**
     * 处理重复文件策略，返回 savePath 或 null（取消）
     */
    async function handleDuplicate(
        song: SongInfo,
        resolved: QualityItem
    ): Promise<string | null> {
        const pathInfo = await musicApi.checkDownloadPath({
            songId: song.id,
            songMid: song.mid,
            songTitle: song.title,
            artist: song.artist,
            album: song.album,
            coverUrl: song.coverUrl,
            qualityFilename: resolved.filename,
            quality: resolved.quality,
        });

        if (!pathInfo.exists) {
            return '';
        }

        const strategy = settingsStore.settings.duplicateStrategy || 'ask';

        if (strategy === 'cancel') {
            notification.info({
                title: '已取消下载',
                description: `歌曲“${song.title}”已存在，取消下载`,
                duration: 3000,
            });
            return null;
        } else if (strategy === 'rename') {
            return pathInfo.suggested_path;
        } else if (strategy === 'overwrite') {
            return '';
        } else {
            // ask
            const action = await askDuplicateAction(song.title);
            if (action === 'cancel') {
                notification.info({
                    title: '已取消下载',
                    description: `歌曲“${song.title}”已存在，取消下载`,
                    duration: 3000,
                });
                return null;
            } else if (action === 'rename') {
                return pathInfo.suggested_path;
            } else {
                return '';
            }
        }
    }

    async function downloadSingle(
        song: SongInfo,
        forceQuality?: Quality
    ): Promise<void> {
        try {
            let quality: Quality
            if (forceQuality) {
                quality = forceQuality
            } else if (settingsStore.settings.defaultQuality === 'ask') {
                try {
                    quality = await askQuality(song.qualities)
                } catch {
                    return
                }
            } else {
                quality = settingsStore.settings.defaultQuality
            }

            const resolved = resolveQualityForSong(song, quality)
            if (!resolved) {
                // 直接创建错误任务
                const taskId = generateTaskId()
                taskStore.addTask({
                    id: taskId,
                    platform: song.platform,
                    songId: song.id,
                    songMid: song.mid,
                    songTitle: song.title,
                    artist: song.artist,
                    album: song.album,
                    coverUrl: song.coverUrl,
                    mediaMid: song.mediaMid,
                    filename: '',
                    quality,
                    status: 'error',
                    errorMsg: '所选音质不可用',
                    fileSize: 0,
                    downloaded: 0,
                    retryCount: 0,
                    addedAt: Date.now(),
                })
                notification.warning({ title: '下载提示', description: `歌曲“${song.title}”无可用音质“${quality}”，已将任务标记为错误` })
                return
            }

            const savePath = await handleDuplicate(song, resolved);
            if (savePath === null) return;

            const taskId = generateTaskId()
            taskStore.addTask({
                id: taskId,
                platform: song.platform,
                songId: song.id,
                songMid: song.mid,
                songTitle: song.title,
                artist: song.artist,
                album: song.album,
                coverUrl: song.coverUrl,
                mediaMid: song.mediaMid,
                filename: resolved.filename,
                quality: resolved.quality,
                status: 'waiting',
                fileSize: resolved.size,
                downloaded: 0,
                retryCount: 0,
                addedAt: Date.now(),
            }, savePath)

            if (settingsStore.settings.jumpToTask) {
                router.push('/task')
            }
        } catch (e: any) {
            console.error('下载失败:', e)
            notification.error({ title: '下载失败', description: e?.message || String(e) })
        }
    }

    async function batchDownload(songs: SongInfo[]): Promise<void> {
        try {
            let quality: Quality
            if (settingsStore.settings.defaultQuality === 'ask') {
                // 取所有歌曲品质的并集作为选项
                const unionMap = new Map<string, QualityItem>()
                for (const song of songs) {
                    for (const q of song.qualities) {
                        if (!unionMap.has(q.quality)) {
                            unionMap.set(q.quality, q)
                        }
                    }
                }
                const unionQualities = Array.from(unionMap.values())
                if (unionQualities.length === 0) {
                    // 所有歌曲都没有可用品质，直接创建错误任务
                    for (const song of songs) {
                        const taskId = generateTaskId()
                        taskStore.addTask({
                            id: taskId,
                            platform: song.platform,
                            songId: song.id,
                            songMid: song.mid,
                            songTitle: song.title,
                            artist: song.artist,
                            album: song.album,
                            coverUrl: song.coverUrl,
                            mediaMid: song.mediaMid,
                            filename: '',
                            quality: '',
                            status: 'error',
                            errorMsg: '无可用音质',
                            fileSize: 0,
                            downloaded: 0,
                            retryCount: 0,
                            addedAt: Date.now(),
                        })
                    }
                    notification.warning({ title: '批量下载', description: '所选歌曲均无可用的音质' })
                    return
                }
                try {
                    quality = await askQuality(unionQualities)
                } catch {
                    return
                }
            } else {
                quality = settingsStore.settings.defaultQuality
            }

            let errorCount = 0
            for (const song of songs) {
                const resolved = resolveQualityForSong(song, quality)
                if (!resolved) {
                    const taskId = generateTaskId()
                    taskStore.addTask({
                        id: taskId,
                        platform: song.platform,
                        songId: song.id,
                        songMid: song.mid,
                        songTitle: song.title,
                        artist: song.artist,
                        album: song.album,
                        coverUrl: song.coverUrl,
                        mediaMid: song.mediaMid,
                        filename: '',
                        quality,
                        status: 'error',
                        errorMsg: '所选音质不可用',
                        fileSize: 0,
                        downloaded: 0,
                        retryCount: 0,
                        addedAt: Date.now(),
                    })
                    errorCount++
                    continue
                }

                const savePath = await handleDuplicate(song, resolved);
                if (savePath === null) continue;

                const taskId = generateTaskId()
                taskStore.addTask({
                    id: taskId,
                    platform: song.platform,
                    songId: song.id,
                    songMid: song.mid,
                    songTitle: song.title,
                    artist: song.artist,
                    album: song.album,
                    coverUrl: song.coverUrl,
                    mediaMid: song.mediaMid,
                    filename: resolved.filename,
                    quality: resolved.quality,
                    status: 'waiting',
                    fileSize: resolved.size,
                    downloaded: 0,
                    retryCount: 0,
                    addedAt: Date.now(),
                }, savePath)
            }

            if (errorCount > 0) {
                notification.warning({ title: '批量下载', description: `${errorCount} 首歌曲无可用音质，已标记为错误` })
            }

            if (settingsStore.settings.jumpToTask) {
                router.push('/task')
            }
        } catch (e: any) {
            console.error('批量下载失败:', e)
            notification.error({ title: '批量下载失败', description: e?.message || String(e) })
        }
    }

    async function retryTask(taskId: string): Promise<void> {
        const task = taskStore.tasks.find((t) => t.id === taskId)
        if (!task || task.status !== 'error') return

        const canRetry = await taskStore.retryTask(taskId)  // 现在等待结果
        if (!canRetry) {
            notification.warning({ title: '重试失败', description: '任务无法重试，已达最大尝试次数或无可降级音质' })
        }
    }

    return {
        downloadSingle,
        batchDownload,
        retryTask,
    }
}