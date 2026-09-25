<template>
    <div class="login-setting">
        <!-- 未登录状态 -->
        <template v-if="!isLoggedIn">
            <n-tabs v-model:value="activeTab" type="line" size="small">
                <n-tab-pane name="qr" tab="扫码登录">
                    <div class="qr-container">
                        <img v-if="qrBase64" :src="'data:image/png;base64,' + qrBase64" alt="二维码" class="qr-img" />
                        <div class="qr-tips">
                            <span v-if="qrStatus === 'waiting'">请使用 QQ 音乐 App 扫码</span>
                            <span v-else-if="qrStatus === 'scanned'">已扫码，请在手机上确认</span>
                            <span v-else-if="qrStatus === 'expired'">二维码已过期，请刷新</span>
                            <span v-else-if="qrStatus === 'error'">登录出错，请重试</span>
                        </div>
                        <n-button size="small" @click="refreshQr" :loading="qrLoading">刷新二维码</n-button>
                    </div>
                </n-tab-pane>
                <n-tab-pane name="manual" tab="手动登录">
                    <n-form :label-placement="isNarrow ? 'top' : 'left'" :label-width="isNarrow ? undefined : 110">
                        <n-form-item label="UIN">
                            <n-input v-model:value="manualUin" placeholder="QQ音乐 UIN（必填）" />
                        </n-form-item>
                        <n-form-item label="Authst">
                            <n-input v-model:value="manualAuthst" placeholder="QQ音乐 Authst（必填）" />
                        </n-form-item>
                        <n-form-item label="RefreshToken">
                            <n-input v-model:value="manualRefreshToken" placeholder="选填，用于刷新登录" />
                        </n-form-item>
                        <n-form-item label="RefreshKey">
                            <n-input v-model:value="manualRefreshKey" placeholder="选填，用于刷新登录" />
                        </n-form-item>
                        <n-form-item label="AccessToken">
                            <n-input v-model:value="manualAccessToken" placeholder="选填，用于刷新登录" />
                        </n-form-item>
                        <n-form-item label="OpenID">
                            <n-input v-model:value="manualOpenid" placeholder="选填，用于刷新登录" />
                        </n-form-item>
                        <n-button type="primary" block @click="handleManualLogin" :loading="manualLoading">
                            登录
                        </n-button>
                    </n-form>
                </n-tab-pane>
            </n-tabs>
        </template>

        <!-- 已登录状态 -->
        <template v-else>
            <div class="logged-in">
                <span>已登录：{{ loginUin }}</span>
                <n-button size="small" type="error" @click="handleLogout">退出登录</n-button>
            </div>
        </template>
    </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { NTabs, NTabPane, NForm, NFormItem, NInput, NButton, useNotification } from 'naive-ui'
import * as musicApi from '../../api/musicApi'
import { useSettingsStore } from '../../stores/settingsStore'
import { useNarrowLayout } from '../../composables/useNarrowLayout'
import { isNativeRuntime } from '../../api/runtimeApi'

const PLATFORM = "qqmusic"

const notification = useNotification()
const settingsStore = useSettingsStore()
const isNarrow = useNarrowLayout()
const isLoggedIn = ref(false)
const loginUin = ref('')
const activeTab = ref('qr')

// 二维码相关
const qrBase64 = ref('')
const qrId = ref('')
const qrStatus = ref('waiting')
const qrLoading = ref(false)
let pollTimer: ReturnType<typeof setInterval> | null = null

// 手动登录
const manualUin = ref('')
const manualAuthst = ref('')
const manualRefreshToken = ref('')
const manualRefreshKey = ref('')
const manualAccessToken = ref('')
const manualOpenid = ref('')
const manualLoading = ref(false)

/** Tauri 仍使用原有设置字段；Web 凭据由独立 Rust 服务保存，不能混入普通设置。 */
function updateNativeCredentials(credentials: musicApi.LoginCredentials | null) {
    if (!isNativeRuntime()) return

    settingsStore.settings.loginUin = credentials?.uin ?? ''
    settingsStore.settings.authst = credentials?.authst ?? ''
    settingsStore.settings.refreshToken = credentials?.refreshToken ?? ''
    settingsStore.settings.refreshKey = credentials?.refreshKey ?? ''
    settingsStore.settings.accessToken = credentials?.accessToken ?? ''
    settingsStore.settings.openid = credentials?.openid ?? ''
}

// 获取新二维码
async function refreshQr() {
    if (pollTimer) clearInterval(pollTimer)
    qrLoading.value = true
    try {
        const res = await musicApi.createQrLogin(PLATFORM)
        // 登录结果可能包含会话信息，避免将其写入浏览器开发者工具日志。
        qrBase64.value = res.qr_base64
        qrId.value = res.qrcode_id
        qrStatus.value = 'waiting'
        startPolling()
    } catch (error) {
        console.error('获取二维码失败:', error)
        qrStatus.value = 'error'
    } finally {
        qrLoading.value = false
    }
}

// 开始轮询登录状态
function startPolling() {
    if (pollTimer) clearInterval(pollTimer)
    console.log('[登录] 开始轮询二维码状态')
    pollTimer = setInterval(async () => {
        try {
            const result = await musicApi.checkQrLogin(PLATFORM, qrId.value)
            qrStatus.value = result.status
            if (result.status === 'confirmed') {
                if (pollTimer) clearInterval(pollTimer)
                if (result.credentials) {
                    updateNativeCredentials(result.credentials)
                }
                isLoggedIn.value = true
                loginUin.value = result.credentials?.uin || ''
                activeTab.value = 'qr'
            } else if (
                result.status === 'expired' ||
                result.status === 'error' ||
                result.status === 'canceled'
            ) {
                if (pollTimer) clearInterval(pollTimer)
                console.log('[登录] 登录流程结束，状态:', result.status)
            }
        } catch (error) {
            console.error('[登录] 轮询登录状态失败:', error)
        }
    }, 2000)
}

// 手动登录
async function handleManualLogin() {
    // 当必填项 UIN 或 Authst 为空时，给用户明确的错误提示。
    if (!manualUin.value || !manualAuthst.value) {
        const errorMsg = 'UIN 和 Authst 为必填项，请完整填写后再登录'
        console.error(`[登录] 手动登录校验失败: ${errorMsg}`)
        notification.error({
            content: errorMsg,
            duration: 3000,
        })
        return
    }
    manualLoading.value = true
    try {
        const creds = await musicApi.loginWithUinAuthst(
            PLATFORM,
            manualUin.value,
            manualAuthst.value,
            manualRefreshToken.value,
            manualRefreshKey.value,
            manualAccessToken.value,
            manualOpenid.value
        )
        updateNativeCredentials(creds)
        isLoggedIn.value = true
        loginUin.value = creds.uin
        activeTab.value = 'qr'
    } catch (error) {
        console.error('[登录] 手动登录失败:', error)
        notification.error({
            content: '登录失败，请检查 UIN、Authst 及选填字段',
            duration: 3000,
        })
    } finally {
        manualLoading.value = false
    }
}

// 退出登录
async function handleLogout() {
    try {
        await musicApi.logout(PLATFORM)
        updateNativeCredentials(null)
        isLoggedIn.value = false
        loginUin.value = ''
        if (pollTimer) clearInterval(pollTimer)
    } catch (error) {
        console.error('退出登录失败:', error)
    }
}

onMounted(async () => {
    try {
        const status = await musicApi.getLoginStatus(PLATFORM)
        isLoggedIn.value = status.logged_in
        loginUin.value = status.uin
    } catch (error) {
        console.error('获取登录状态失败:', error)
    }
})

onUnmounted(() => {
    if (pollTimer) clearInterval(pollTimer)
})
</script>

<style scoped>
.login-setting {
    width: 100%;
    min-width: 0;
}

.qr-container {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    margin-bottom: 12px;
}

.qr-img {
    width: 160px;
    height: 160px;
    /* 深色模式下二维码与页面背景易融合，添加白底和内边距形成白色边框，便于扫描定位 */
    background-color: #fff;
    padding: 8px;
    border-radius: 8px;
    box-sizing: border-box;
}

.qr-tips {
    font-size: 13px;
    color: var(--color-text-secondary);
    text-align: center;
}

.logged-in {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 12px;
}

.logged-in > span {
    min-width: 0;
    overflow-wrap: anywhere;
}

.logged-in :deep(.n-button) {
    flex-shrink: 0;
}
</style>
