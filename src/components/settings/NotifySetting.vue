<template>
    <template v-if="isNarrow">
        <!-- 移动端：开关行内布局，与其他开关组件保持一致 -->
        <div class="setting-row">
            <span class="setting-label">下载完成后发送系统通知</span>
            <n-switch :value="settingsStore.settings.notifyOnComplete" @update:value="handleNotifyToggle" />
        </div>
    </template>
    <template v-else>
        <!-- 桌面端：原有表单布局 -->
        <n-form-item label="下载完成后发送系统通知">
            <n-switch :value="settingsStore.settings.notifyOnComplete" @update:value="handleNotifyToggle" />
        </n-form-item>
    </template>
</template>

<script setup lang="ts">
import { useNarrowLayout } from '../../composables/useNarrowLayout'
import { NFormItem, NSwitch } from 'naive-ui'
import { useSettingsStore } from '../../stores/settingsStore'
import { requestNotificationPermission, checkNotificationPermission } from '../../api/musicApi'

const settingsStore = useSettingsStore()

// 移动端判断
const isNarrow = useNarrowLayout()

const notify = () => window.$notify

// ===== 处理通知开关切换 =====
// 用户开启通知时，必须先获得系统通知权限，否则开关保持关闭状态，
// 避免设置与实际权限不一致，同时在权限被拒绝时给用户明确提示。
async function handleNotifyToggle(val: boolean) {
    if (!val) {
        // 用户关闭开关，直接更新设置
        settingsStore.settings.notifyOnComplete = false
        return
    }

    // ===== 先检查权限状态，避免重复请求导致卡死 =====
    // 用户再次开启开关时，若权限已授予则直接允许，不再触发系统请求。
    try {
        const alreadyGranted = await checkNotificationPermission()
        if (alreadyGranted) {
            settingsStore.settings.notifyOnComplete = true
            return
        }

        // 未授予：请求权限
        const granted = await requestNotificationPermission()
        if (granted) {
            settingsStore.settings.notifyOnComplete = true
        } else {
            // 权限被拒绝：保持关闭状态，并提示用户
            settingsStore.settings.notifyOnComplete = false
            notify()?.warning({
                title: '通知权限',
                description: '通知权限被拒绝，请在系统设置中允许',
                duration: 3000
            })
        }
    } catch (error) {
        // 请求权限过程中发生错误：保持关闭状态，并提示用户
        settingsStore.settings.notifyOnComplete = false
        console.error('请求通知权限失败:', error)
        notify()?.error({
            title: '请求失败',
            description: '请求通知权限失败，请稍后重试',
            duration: 3000
        })
    }
}
</script>

<style scoped>
.setting-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: 16px;
}

.setting-label {
    font-size: 14px;
    color: var(--n-text-color);
}
</style>
