<template>
    <n-form-item label="下载目录">
        <template v-if="!isAndroid">
            <n-input-group>
                <n-input :value="settingsStore.settings.downloadDir" readonly placeholder="请选择下载目录" />
                <n-button type="primary" @click="selectDirectory">选择</n-button>
            </n-input-group>
        </template>
        <template v-else>
            <div class="android-dir-setting">
                <n-button type="primary" @click="selectSafFolder">
                    选择 SAF 文件夹
                </n-button>
                <div class="current-dir">
                    <n-text v-if="settingsStore.settings.safFolderName" type="info">
                        当前 SAF 文件夹：{{ settingsStore.settings.safFolderName }}
                    </n-text>
                    <n-text v-else depth="3">
                        默认下载目录：{{ settingsStore.settings.downloadDir }}
                    </n-text>
                </div>
            </div>
        </template>
    </n-form-item>
</template>

<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { NFormItem, NInput, NInputGroup, NButton, NText } from 'naive-ui'
import { open } from '@tauri-apps/plugin-dialog'
import { invoke } from '@tauri-apps/api/core'
// 导入 OS 插件，用于检测 Android 平台
import { platform } from '@tauri-apps/plugin-os'
import { useSettingsStore } from '../../stores/settingsStore'

const settingsStore = useSettingsStore()

// 不再使用 UA 判断，改用 platform() 异步获取，初始值为 false
const isAndroid = ref(false)

// Android 端初始化：异步获取平台信息，若为 Android 且未选择 SAF，则确保使用默认下载目录
onMounted(async () => {
    try {
        const currentPlatform = await platform()
        isAndroid.value = currentPlatform === 'android'
    } catch (error) {
        console.warn('获取平台信息失败，默认按非 Android 处理', error)
        isAndroid.value = false
    }

    if (isAndroid.value && !settingsStore.settings.safFolderUri) {
        await settingsStore.getDefaultDownloadDir()
    }
})

// 桌面端目录选择
async function selectDirectory() {
    // 仅桌面端调用
    try {
        const selected = await open({
            directory: true,
            multiple: false,
            title: '选择下载目录',
        })
        if (selected && typeof selected === 'string') {
            settingsStore.settings.downloadDir = selected
        }
    } catch (error) {
        console.error('选择目录失败:', error)
    }
}

async function selectSafFolder() {
    try {
        const json = await invoke<string>('pick_saf_folder')
        if (!json) {
            console.log('用户取消选择 SAF 文件夹')
            return
        }
        // 存储完整 JSON
        settingsStore.settings.safFolderUri = json

        // 解析 JSON 获取 URI 最后一段作为显示名称
        try {
            const parsed = JSON.parse(json)
            settingsStore.settings.safFolderName = parsed.uri.split('/').pop() || parsed.uri
        } catch {
            settingsStore.settings.safFolderName = 'SAF 文件夹'
        }
        settingsStore.settings.downloadDir = 'saf://'
    } catch (error) {
        console.error('选择 SAF 文件夹失败:', error)
    }
}
</script>

<style scoped>
.android-dir-setting {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
    width: 100%;
    align-items: flex-start;
}

.current-dir {
    font-size: 13px;
    max-width: 100%;
    overflow-wrap: anywhere;
    line-height: 1.6;
}
</style>
