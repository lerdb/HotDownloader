import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'

/** 桌面端使用原生目录对话框；取消选择时返回 null。 */
export async function chooseDownloadDirectory(): Promise<string | null> {
    const selected = await open({
        directory: true,
        multiple: false,
        title: '选择下载目录',
    })
    return typeof selected === 'string' ? selected : null
}

/** Android SAF 插件返回序列化的 URI；空值表示用户取消。 */
export function pickSafFolder(): Promise<string> {
    return invoke<string>('pick_saf_folder')
}

export function openFileLocation(path: string): Promise<void> {
    return invoke<void>('open_file_location', { path })
}
