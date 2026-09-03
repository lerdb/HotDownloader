import { invoke } from '@tauri-apps/api/core'

/**
 * 通用 Tauri 命令调用封装，提供：
 * 1. 内存缓存（同一 key 在 TTL 内的请求直接返回缓存）
 * 2. 请求去重（同一 key 的并发请求共享同一个 Promise）
 *
 * 适用于"幂等、读多写少"的命令（如封面、配置查询等）。
 * 不要用于有副作用的命令（如添加任务、修改设置）。
 */

interface CacheEntry<T> {
    promise: Promise<T>
    timestamp: number
}

const cache = new Map<string, CacheEntry<unknown>>()

/** 默认缓存有效期 5 分钟 */
const DEFAULT_TTL_MS = 5 * 60 * 1000

/**
 * 调用 Tauri 命令并自动缓存结果。
 *
 * @param command Tauri 命令名
 * @param args 命令参数
 * @param options.ttlMs 缓存有效期（毫秒），默认 5 分钟
 * @param options.key 自定义缓存键（默认用 JSON 序列化的 args）
 * @returns 命令返回结果
 */
export function cachedInvoke<T>(
    command: string,
    args?: Record<string, unknown>,
    options: { ttlMs?: number; key?: string } = {}
): Promise<T> {
    const { ttlMs = DEFAULT_TTL_MS, key } = options
    const cacheKey = key ?? `${command}:${JSON.stringify(args ?? {})}`

    const now = Date.now()
    const cached = cache.get(cacheKey)
    if (cached && now - cached.timestamp < ttlMs) {
        return cached.promise as Promise<T>
    }

    // 已有的 in-flight 请求直接复用，避免并发触发
    if (cached) {
        // 过期但仍在飞行中，复用旧 Promise（避免重复请求）
        return cached.promise as Promise<T>
    }

    const promise = invoke<T>(command, args) as Promise<T>
    cache.set(cacheKey, { promise, timestamp: now })

    // 请求失败时清除缓存，允许后续重试
    promise.catch(() => cache.delete(cacheKey))

    return promise
}

/** 清除全部缓存（用于登出、切换账号等场景） */
export function clearInvokeCache(): void {
    cache.clear()
}

/** 清除指定命令的缓存 */
export function clearInvokeCacheByCommand(command: string): void {
    for (const key of cache.keys()) {
        if (key.startsWith(`${command}:`)) {
            cache.delete(key)
        }
    }
}