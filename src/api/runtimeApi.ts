import { isTauri } from '@tauri-apps/api/core'
import { getCurrentWindow } from '@tauri-apps/api/window'
import { platform } from '@tauri-apps/plugin-os'

/** 业务页面通过此入口判断是否具备原生窗口和文件操作能力。 */
export function isNativeRuntime(): boolean {
    return isTauri()
}

/** 原生平台标识只在此处读取，UI 不直接依赖 Tauri OS 插件。 */
export async function getRuntimePlatform(): Promise<string> {
    return isTauri() ? platform() : 'web'
}

/**
 * 拦截桌面窗口关闭请求。回调返回 true 时由适配层关闭窗口，
 * false 时保持窗口打开；调用方无需接触 Tauri 的 CloseRequestedEvent。
 */
export async function subscribeCloseRequest(
    shouldClose: () => Promise<boolean>,
): Promise<() => void> {
    // 浏览器关闭标签页不会终止后端任务，也没有 Tauri 的窗口关闭事件。
    if (!isTauri()) {
        return () => undefined
    }

    const appWindow = getCurrentWindow()
    let unlisten: (() => void) | undefined

    unlisten = await appWindow.onCloseRequested(async event => {
        event.preventDefault()
        if (await shouldClose()) {
            // destroy 会再次触发关闭事件，先取消监听以避免重复询问。
            unlisten?.()
            await appWindow.destroy()
        }
    })

    return () => unlisten?.()
}

/** 安全区域插件会轮询 Tauri 对象，普通浏览器中不能直接静态导入。 */
export async function initializeNativeSafeArea(): Promise<void> {
    if (isTauri()) {
        await import('@sahil-vartak/tauri-plugin-safe-area-insets-css-api')
    }
}
