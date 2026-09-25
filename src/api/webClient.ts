import { reactive } from 'vue'

/** Web 端只把访问令牌留在当前浏览器会话；页面刷新后可继续使用，关闭浏览器即清除。 */
const TOKEN_KEY = 'hotdownloader-web-token'

export const webSession = reactive({
    authorized: false,
    checking: false,
    error: '',
})

let accessToken = sessionStorage.getItem(TOKEN_KEY) ?? ''

export function webHeaders(): HeadersInit {
    return accessToken ? { Authorization: `Bearer ${accessToken}` } : {}
}

export async function webRequest<T>(path: string, options: RequestInit = {}): Promise<T> {
    const headers = new Headers(options.headers)
    if (accessToken) {
        headers.set('Authorization', `Bearer ${accessToken}`)
    }
    if (options.body !== undefined) {
        headers.set('Content-Type', 'application/json')
    }

    const response = await fetch(path, { ...options, headers, cache: 'no-store' })
    if (response.status === 401) {
        webSession.authorized = false
        throw new Error('访问令牌无效或已失效')
    }
    if (!response.ok) {
        const body = await response.json().catch(() => null) as { error?: string } | null
        throw new Error(body?.error ?? `请求失败：HTTP ${response.status}`)
    }
    return response.json() as Promise<T>
}

export async function authorizeWeb(token = accessToken): Promise<boolean> {
    webSession.checking = true
    webSession.error = ''
    accessToken = token.trim()
    try {
        // 任务快照接口只读，适合在启动和令牌输入后验证访问权限。
        await webRequest<unknown[]>('/api/tasks')
        webSession.authorized = true
        if (accessToken) {
            sessionStorage.setItem(TOKEN_KEY, accessToken)
        } else {
            sessionStorage.removeItem(TOKEN_KEY)
        }
        return true
    } catch (error) {
        webSession.authorized = false
        webSession.error = error instanceof Error ? error.message : String(error)
        sessionStorage.removeItem(TOKEN_KEY)
        return false
    } finally {
        webSession.checking = false
    }
}
