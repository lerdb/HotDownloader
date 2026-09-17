import { ref, computed, onMounted } from 'vue'
// Markdown 渲染依赖
import { marked } from 'marked'
import DOMPurify from 'dompurify'
// 语义化版本比较库 semver
import semver from 'semver'
// Tauri OS 插件，用于获取当前平台信息
import { platform } from '@tauri-apps/plugin-os'
import { checkForUpdate } from '../api/musicApi'
import { formatFileSize } from '../utils/format'
import type { UpdateInfo } from '../types'

export function useUpdateChecker() {
    // 检查更新相关状态
    const checkingUpdate = ref(false)
    const updateInfo = ref<UpdateInfo | null>(null)
    const showUpdateModal = ref(false)

    // 当前平台标识，用于过滤下载资产
    const currentPlatform = ref<string>('')

    // 根据当前平台过滤下载资产列表，实现平台相关的安装包显示，提升用户体验
    const filteredAssets = computed(() => {
        if (!updateInfo.value || !updateInfo.value.assets) return []
        const platformStr = currentPlatform.value
        if (!platformStr) return updateInfo.value.assets // 平台尚未获取，显示全部
        const assets = updateInfo.value.assets
        if (platformStr === 'android') {
            // Android 平台仅展示 release 版本的 APK
            return assets.filter(a => a.name.endsWith('.apk') && a.name.toLowerCase().includes('release'))
        } else if (platformStr === 'windows') {
            return assets.filter(a => a.name.endsWith('.exe') || a.name.endsWith('.msi'))
        } else if (platformStr === 'macos' || platformStr === 'darwin') {
            return assets.filter(a => a.name.endsWith('.dmg'))
        } else if (platformStr === 'linux') {
            return assets.filter(a => a.name.endsWith('.deb') || a.name.endsWith('.rpm') || a.name.endsWith('.AppImage'))
        }
        // 其他平台显示全部
        return assets
    })

    // 支持更新内容 Markdown 渲染，提升可读性
    function renderMarkdown(markdown: string): string {
        if (!markdown) return ''
        // 配置 marked 渲染选项：启用 GFM（GitHub Flavored Markdown）和换行转换
        marked.setOptions({ gfm: true, breaks: true })
        const rawHtml = marked.parse(markdown) as string
        // 使用 DOMPurify 过滤，允许常见安全标签
        return DOMPurify.sanitize(rawHtml, { USE_PROFILES: { html: true } })
    }

    // 检查更新功能优化，使用成熟库替代手写比较，正确处理预发布版本等复杂情况
    // 去除 tag_name 可能的前缀 v，然后使用 semver.gt() 判断 latest 是否大于 current
    const isNewVersion = computed(() => {
        if (!updateInfo.value) return false
        const current = updateInfo.value.current_version
        const latest = updateInfo.value.tag_name.replace(/^v/, '')
        // 使用 semver.gt 比较两个版本号，返回是否 latest > current
        // 注意：semver 库会处理预发布版本的优先级规则
        return semver.valid(current) !== null && semver.valid(latest) !== null && semver.gt(latest, current)
    })

    // 显示错误通知：使用全局 $notify（NavLayout 已挂载），避免依赖未提供的 message provider
    function showErrorNotification(message: string) {
        const notify = () => window.$notify
        if (typeof window !== 'undefined' && notify()) {
            notify()?.error({ title: '错误', description: message, duration: 3000 })
        } else {
            console.error(message)
        }
    }

    // 后台静默检查更新：仅更新数据，不弹窗，失败仅记录日志
    async function checkUpdateInBackground() {
        try {
            const info = await checkForUpdate()
            updateInfo.value = info
        } catch (error) {
            console.warn('自动检查更新失败:', error)
            // 静默失败，不打扰用户
        }
    }

    // 手动点击检查更新：成功弹窗展示，失败弹出错误通知
    async function handleCheckUpdate() {
        if (checkingUpdate.value) return
        checkingUpdate.value = true
        try {
            const info = await checkForUpdate()
            updateInfo.value = info
            showUpdateModal.value = true
        } catch (error) {
            console.error('检查更新失败:', error)
            showErrorNotification('检查更新失败，请稍后重试')
        } finally {
            checkingUpdate.value = false
        }
    }

    // 组件挂载时自动获取平台信息和静默检查更新
    onMounted(async () => {
        // 使用 async/await 获取当前平台信息，避免类型不匹配和代码繁琐
        try {
            currentPlatform.value = await platform()
        } catch (err) {
            console.warn('获取平台信息失败，将显示所有安装包', err)
            currentPlatform.value = ''
        }

        // 自动静默检查更新
        await checkUpdateInBackground()
    })

    return {
        checkingUpdate,
        updateInfo,
        showUpdateModal,
        currentPlatform,
        filteredAssets,
        isNewVersion,
        renderMarkdown,
        formatFileSize,
        handleCheckUpdate,
        checkUpdateInBackground,
    }
}
