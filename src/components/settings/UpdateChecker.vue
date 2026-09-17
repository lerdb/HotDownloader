<template>
    <!-- 检查更新入口（移动端与桌面端通用） -->
    <div class="check-update-entry">
        <n-button :loading="checkingUpdate" @click="handleCheckUpdate">
            检查更新
        </n-button>
        <span v-if="updateInfo && !checkingUpdate" class="update-status-text">
            {{ isNewVersion ? '发现新版本' : '已是最新版本' }}
        </span>
    </div>

    <!-- 更新信息弹窗：桌面端最大宽度 600px，移动端左右留白 16px -->
    <n-modal v-model:show="showUpdateModal" preset="card" class="update-modal" :title="isNewVersion ? '发现新版本' : '检查更新'"
        style="max-width: 600px; width: calc(100% - 32px);">
        <div v-if="updateInfo" class="update-content">
            <p class="version-line">
                当前版本：{{ updateInfo.current_version }}
                <template v-if="isNewVersion">
                    ｜ 最新版本：{{ updateInfo.tag_name }}
                </template>
            </p>
            <p v-if="updateInfo.published_at" class="publish-date">
                发布时间：{{ updateInfo.published_at }}
            </p>
            <div class="update-body">
                <n-text class="body-label">更新内容：</n-text>
                <!-- 使用 v-html 渲染 Markdown 解析后的 HTML，提升可读性 -->
                <!-- 调用 renderMarkdown 函数生成安全 HTML；若无内容则显示默认文本 -->
                <div class="body-text markdown-body" v-html="renderMarkdown(updateInfo.body) || '<p>（无更新说明）</p>'">
                </div>
            </div>
            <!-- 下载安装包直链区域（当存在匹配当前平台的 assets 时显示） -->
            <!-- 检查更新功能优化，只显示当前平台可用的安装包，避免用户下载错误文件 -->
            <div v-if="filteredAssets.length > 0" class="assets-section">
                <n-text class="body-label">下载安装包：</n-text>
                <div class="asset-list">
                    <!-- 使用 filteredAssets 计算属性，其根据 currentPlatform 过滤原始 assets -->
                    <a v-for="asset in filteredAssets" :key="asset.name" class="asset-link"
                        :href="asset.browser_download_url" target="_blank" rel="noopener noreferrer">
                        {{ asset.name }}（{{ formatFileSize(asset.size) }}）
                    </a>
                </div>
            </div>
        </div>
        <template #footer>
            <div v-if="updateInfo" class="modal-actions">
                <n-button type="primary" @click="showUpdateModal = false">关闭</n-button>
                <n-button v-if="updateInfo.html_url" tag="a" :href="updateInfo.html_url" target="_blank">
                    前往发布页
                </n-button>
            </div>
        </template>
    </n-modal>
</template>

<script setup lang="ts">
import { useUpdateChecker } from '../../composables/useUpdateChecker'

const {
    checkingUpdate,
    updateInfo,
    showUpdateModal,
    filteredAssets,
    isNewVersion,
    renderMarkdown,
    formatFileSize,
    handleCheckUpdate,
} = useUpdateChecker()
</script>

<style scoped>
/* 检查更新入口样式：与关于入口类似，居中 */
.check-update-entry {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 12px;
    flex-wrap: wrap;
    margin-top: 24px;
}

.update-status-text {
    font-size: 13px;
    color: var(--color-text-secondary);
}

/* 更新信息弹窗内部样式 */
/* Modal 渲染到 body，通过专属类名限制弹窗高度和内容滚动区域 */
:global(.update-modal) {
    max-height: calc(100vh - 32px - var(--safe-area-top) - var(--safe-area-bottom));
    max-height: calc(100dvh - 32px - var(--safe-area-top) - var(--safe-area-bottom));
}

:global(.update-modal > .n-card-content) {
    min-height: 0;
    overflow-y: auto;
}

.update-content {
    min-width: 0;
    overflow-wrap: anywhere;
    line-height: 1.6;
    /* 增加内容区上下空白，使弹窗不显得拥挤 */
    padding: 8px 0;
}

.version-line {
    font-size: 15px;
    font-weight: 500;
    margin-bottom: 8px;
}

.publish-date {
    font-size: 13px;
    color: var(--color-text-secondary);
    margin-bottom: 16px;
}

.update-body {
    margin-bottom: 20px;
}

.body-label {
    font-weight: 500;
}

.body-text {
    margin-top: 4px;
    /* 适配 Markdown 渲染后的 HTML 内容，取消 pre-wrap 改为正常换行 */
    color: var(--color-text);
    max-height: 300px;
    overflow-y: auto;
    line-height: 1.6;
}

/* Markdown 内容的基础样式，保证标题、列表、代码块等可读 */
.markdown-body :deep(h1),
.markdown-body :deep(h2),
.markdown-body :deep(h3),
.markdown-body :deep(h4) {
    margin: 12px 0 8px;
    font-weight: 600;
}

.markdown-body :deep(p) {
    margin: 8px 0;
}

.markdown-body :deep(ul),
.markdown-body :deep(ol) {
    padding-left: 24px;
    margin: 8px 0;
}

.markdown-body :deep(code) {
    background-color: var(--bg-body);
    padding: 2px 4px;
    border-radius: 3px;
    font-size: 0.9em;
}

.markdown-body :deep(pre) {
    background-color: var(--bg-body);
    padding: 12px;
    border-radius: 6px;
    overflow-x: auto;
}

.markdown-body :deep(pre code) {
    background: none;
    padding: 0;
}

.markdown-body :deep(img) {
    max-width: 100%;
    height: auto;
}

.markdown-body :deep(table) {
    display: block;
    max-width: 100%;
    overflow-x: auto;
}

.markdown-body :deep(a) {
    color: var(--color-primary);
}

/* 资产列表样式 */
.assets-section {
    margin-bottom: 20px;
}

.asset-list {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin-top: 8px;
}

.asset-link {
    overflow-wrap: anywhere;
    color: var(--color-primary);
    text-decoration: none;
    font-size: 14px;
    transition: opacity 0.2s;
}

.asset-link:hover {
    opacity: 0.8;
    text-decoration: underline;
}

.modal-actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    flex-wrap: wrap;
}

@media (max-width: 767px) {
    :global(.update-modal > .n-card-header),
    :global(.update-modal > .n-card-content),
    :global(.update-modal > .n-card__footer) {
        padding-left: 12px;
        padding-right: 12px;
    }

    .modal-actions :deep(.n-button) {
        min-height: 44px;
    }
}
</style>
