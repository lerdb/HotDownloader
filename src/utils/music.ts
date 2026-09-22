/**
 * 选取并校验歌手或专辑详情接口使用的标识。
 * @param platform 平台标识：`qqmusic` 或 `kuwo`。
 * @param id 数字 ID 的字符串形式，供酷我使用。
 * @param mid QQ 音乐 MID。
 * @returns 有效的详情标识；平台不受支持或标识无效时返回空字符串。
 */
export function getMusicEntityId(platform: string, id = '', mid = ''): string {
    if (platform === 'qqmusic') {
        return /^[a-zA-Z0-9]+$/.test(mid) && mid !== '0' ? mid : ''
    }
    if (platform === 'kuwo') {
        return /^\d+$/.test(id) && /[1-9]/.test(id) ? id : ''
    }
    return ''
}
