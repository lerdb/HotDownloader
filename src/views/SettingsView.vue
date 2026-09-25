<template>
    <div class="settings-view" :class="{ 'is-narrow': isNarrow }">
        <!-- 移动端：分组纵向布局；桌面端：原有左右分栏表单。
             共用组件实例，缩放窗口时保留登录输入和弹窗中的编辑草稿。 -->
        <!-- 账号设置：独立分类，位于基本设置上方，增加底部间距避免与下方黏连 -->
        <div class="settings-section account-section">
            <h2 class="section-title">账号设置</h2>
            <n-form :label-placement="isNarrow ? 'top' : 'left'">
                <LoginSetting />
            </n-form>
        </div>

        <div class="settings-section">
            <h2 class="section-title">基本设置</h2>
            <n-form :label-placement="isNarrow ? 'top' : 'left'" :label-width="isNarrow ? undefined : 180">
                <QualitySetting />
                <DowngradeSetting />
                <ClearHistoryButton />
            </n-form>
        </div>

        <div class="settings-section">
            <h2 class="section-title">下载设置</h2>
            <n-form :label-placement="isNarrow ? 'top' : 'left'" :label-width="isNarrow ? undefined : 180">
                <DirectorySetting />
                <NamingTemplate />
                <ArtistSeparator />
                <NamingPreview />
                <WriteMetadataSetting />
                <DownloadLrcSetting />
                <ConcurrencySetting />
                <JumpToTaskSetting />
                <DuplicateStrategySetting />
                <NotifySetting v-if="native" />
            </n-form>
        </div>

        <!-- 检查更新组件 -->
        <UpdateChecker v-if="native" />

        <!-- 关于入口（始终位于页面底部） -->
        <div class="about-entry">
            <n-button text @click="goAbout">关于 HotDownloader</n-button>
        </div>
    </div>
</template>

<script setup lang="ts">
import { useNarrowLayout } from '../composables/useNarrowLayout'
import { useRouter } from 'vue-router'
import { NForm, NButton } from 'naive-ui'
import QualitySetting from '../components/settings/QualitySetting.vue'
import DowngradeSetting from '../components/settings/DowngradeSetting.vue'
import DirectorySetting from '../components/settings/DirectorySetting.vue'
import NamingTemplate from '../components/settings/NamingTemplate.vue'
import ArtistSeparator from '../components/settings/ArtistSeparator.vue'
import NamingPreview from '../components/settings/NamingPreview.vue'
import ConcurrencySetting from '../components/settings/ConcurrencySetting.vue'
import JumpToTaskSetting from '../components/settings/JumpToTaskSetting.vue'
import ClearHistoryButton from '../components/settings/ClearHistoryButton.vue'
import WriteMetadataSetting from '../components/settings/WriteMetadataSetting.vue'
import DownloadLrcSetting from '../components/settings/DownloadLrcSetting.vue'
import LoginSetting from '../components/settings/LoginSetting.vue'
import DuplicateStrategySetting from '../components/settings/DuplicateStrategySetting.vue'
import NotifySetting from '../components/settings/NotifySetting.vue'
import UpdateChecker from '../components/settings/UpdateChecker.vue'
import { isNativeRuntime } from '../api/runtimeApi'

const router = useRouter()

// 移动端响应式布局状态
const isNarrow = useNarrowLayout()
const native = isNativeRuntime()

function goAbout() {
    router.push('/settings/about')
}
</script>

<style scoped>
.settings-view {
    width: 100%;
    max-width: 800px;
    min-width: 0;
    /* 让设置页占满父容器高度，使用 flex 列布局 */
    display: flex;
    flex-direction: column;
    min-height: 100%;
}

/* 移动端移除最大宽度限制，撑满父容器 */
.settings-view.is-narrow {
    max-width: none;
}

.settings-section {
    margin-bottom: 16px;
    padding: 20px;
    min-width: 0;
    background: var(--bg-sidebar);
    border: 1px solid var(--border-color);
    border-radius: 8px;
}

.account-section {
    margin-bottom: 20px;
}

.settings-section+.settings-section {
    padding-top: 20px;
}

.section-title {
    font-size: 16px;
    font-weight: 600;
    margin-bottom: 12px;
    color: var(--color-text);
}

.about-entry {
    /* 将关于入口推到底部 */
    margin-top: auto;
    padding-top: 24px;
    text-align: center;
}

/* 统一子组件的表单收缩和行间距，长标签与路径在自己的区域内换行 */
.settings-view :deep(.n-form-item-blank),
.settings-view :deep(.n-input-group) {
    min-width: 0;
}

.settings-view :deep(.setting-row) {
    gap: 12px;
    min-height: 44px;
    flex-wrap: wrap;
}

.settings-view :deep(.setting-label) {
    color: var(--color-text);
    flex: 1;
    min-width: 140px;
}

.settings-view :deep(.setting-row .n-switch) {
    flex-shrink: 0;
}

.settings-view :deep(.setting-row .n-input-number) {
    width: 132px;
}

@media (max-width: 767px) {
    .settings-section,
    .settings-section + .settings-section {
        padding: 16px 12px;
    }

    .settings-view :deep(.n-button) {
        min-height: 44px;
    }
}
</style>
