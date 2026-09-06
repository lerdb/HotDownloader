/**
 * 格式化播放量显示。
 * 规则：
 * - >= 1亿：显示为 X.X亿（保留一位小数）
 * - >= 1万：显示为 X.X万（保留一位小数）
 * - < 1万：直接显示数字
 *
 * @param count 播放量数值
 * @returns 格式化后的字符串
 */
export function formatPlayCount(count: number): string {
    if (count >= 100000000) {
        return (count / 100000000).toFixed(1) + '亿'
    } else if (count >= 10000) {
        return (count / 10000).toFixed(1) + '万'
    }
    return count.toString()
}

/**
 * 格式化速度 (bytes/s) 为人类可读字符串。
 * 规则：
 * - 0 返回空字符串
 * - 根据数值大小自动选择 B/s、KB/s、MB/s、GB/s 单位
 * - 保留一位小数
 *
 * @param bytesPerSec 速度（字节/秒）
 * @returns 格式化后的字符串
 */
export function formatSpeed(bytesPerSec: number): string {
    if (bytesPerSec === 0) return ''
    const units = ['B/s', 'KB/s', 'MB/s', 'GB/s']
    let unitIndex = 0
    let value = bytesPerSec
    while (value >= 1024 && unitIndex < units.length - 1) {
        value /= 1024
        unitIndex++
    }
    return `${value.toFixed(1)} ${units[unitIndex]}`
}

/**
 * 将字节大小格式化为人类可读字符串。
 * 规则：
 * - 0 显示 "0 B"
 * - 根据数值大小自动选择 B、KB、MB、GB、TB 单位
 * - 保留两位小数
 *
 * @param bytes 字节数
 * @returns 格式化后的字符串
 */
export function formatFileSize(bytes: number): string {
    if (bytes === 0) return '0 B'
    const units = ['B', 'KB', 'MB', 'GB', 'TB']
    const i = Math.floor(Math.log(bytes) / Math.log(1024))
    const value = bytes / Math.pow(1024, i)
    return `${value.toFixed(2)} ${units[i]}`
}