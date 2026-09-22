import { computed, ref, watch } from 'vue'
import { useRoute, useRouter, type LocationQueryRaw } from 'vue-router'
import type { AlbumInfo, ArtistInfo, ArtistReference, SongInfo } from '../types'
import { getMusicEntityId } from '../utils/music'

const backLabels: Record<string, string> = {
    '/search': '返回搜索',
    '/artist': '返回歌手',
    '/album': '返回专辑',
    '/playlist': '返回歌单',
}

/**
 * 校验音乐页面的返回地址。
 * @param target 待校验的路由地址。
 * @returns 搜索、歌手、专辑或歌单页面的地址；无效值返回 `/search`。
 */
function musicBackTarget(target: unknown): string {
    return typeof target === 'string' && /^\/(search|artist|album|playlist)(\?|$)/.test(target)
        ? target : '/search'
}

/**
 * 提供歌手、专辑详情的跳转方法及返回按钮状态。
 * 跳转时记录来源页；返回时优先使用匹配的历史记录，否则替换为来源路由。
 */
export function useMusicNavigation() {
    const router = useRouter()
    const route = useRoute()
    const backTarget = ref('/search')

    // history.state 不具备响应性，需要在路由切换后重新读取来源地址。
    watch(() => route.fullPath, () => {
        backTarget.value = musicBackTarget(router.options.history.state.musicReturnTo ?? route.query.returnTo)
    }, { immediate: true })

    const backLabel = computed(() => {
        const path = router.resolve(backTarget.value).path
        return backLabels[path] || '返回'
    })

    function open(path: '/artist' | '/album', platform: string, id: string, query: LocationQueryRaw) {
        if (!getMusicEntityId(platform, id, id)) return
        if (route.path === path && route.query.platform === platform && route.query.id === id) return
        return router.push({
            path,
            query: { ...query, platform, id },
            state: { musicReturnTo: route.fullPath },
        })
    }

    /** 打开歌手详情；artist.id 为 QQ 音乐 MID 或酷我数字 ID 字符串。 */
    function openArtist(platform: string, artist: Pick<ArtistInfo, 'id' | 'name'> & Partial<ArtistInfo>) {
        return open('/artist', platform, artist.id, {
            name: artist.name,
            cover: artist.coverUrl,
            alias: artist.alias,
            region: artist.region,
            songs: artist.songCount,
            albums: artist.albumCount,
        })
    }

    /** 从歌手关联信息中选取平台所需标识并打开详情。 */
    function openRelatedArtist(platform: string, artist: ArtistReference) {
        return openArtist(platform, {
            id: getMusicEntityId(platform, artist.id, artist.mid),
            name: artist.name,
        })
    }

    /** 打开专辑详情；album.id 为 QQ 音乐 MID 或酷我数字 ID 字符串。 */
    function openAlbum(platform: string, album: Pick<AlbumInfo, 'id' | 'name'> & Partial<AlbumInfo>) {
        return open('/album', platform, album.id, {
            name: album.name,
            artist: album.artist,
            date: album.publishDate,
        })
    }

    function openSongAlbum(song: SongInfo) {
        return openAlbum(song.platform, {
            id: getMusicEntityId(song.platform, song.albumId, song.albumMid),
            name: song.album,
        })
    }

    function goBack() {
        if (router.options.history.state.back === backTarget.value) {
            router.back()
        } else {
            return router.replace(backTarget.value)
        }
    }

    return { openArtist, openRelatedArtist, openAlbum, openSongAlbum, goBack, backLabel }
}
