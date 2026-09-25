import { invoke } from '@tauri-apps/api/core'
import { open } from '@tauri-apps/plugin-dialog'
import { isTauri } from '@tauri-apps/api/core'

/** 桌面端使用原生目录对话框；取消选择时返回 null。 */
export async function chooseDownloadDirectory(): Promise<string | null> {
    if (!isTauri()) {
        // 浏览器无法选择服务器上的路径；服务端目录由部署环境指定。
        return null
    }
    const selected = await open({
        directory: true,
        multiple: false,
        title: '选择下载目录',
    })
    return typeof selected === 'string' ? selected : null
}

/** Android SAF 插件返回序列化的 URI；空值表示用户取消。 */
export function pickSafFolder(): Promise<string> {
    if (!isTauri()) {
        return Promise.reject(new Error('网页端不支持 Android SAF'))
    }
    return invoke<string>('pick_saf_folder')
}

export function openFileLocation(path: string): Promise<void> {
    if (!isTauri()) {
        return Promise.reject(new Error(`文件保存在服务器：${path}`))
    }
    return invoke<void>('open_file_location', { path })
}
