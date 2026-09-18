<template>
    <div class="about-view">
        <!-- 在关于页面左上角添加返回按钮，提供明确的返回导航入口 -->
        <n-button @click="goBack">返回</n-button>
        <div class="about-card">
            <h1 class="app-title">HotDownloader</h1>
            <!-- 直接使用注入的变量，不再硬编码 -->
            <div class="app-version">版本 {{ version }}</div>
            <!-- 更新为与 README 一致的跨平台描述 -->
            <p class="app-description">
                基于 Tauri 2 + Vue 3 的跨平台音乐下载应用，支持桌面端（Windows/macOS/Linux）与 Android 端，提供搜索、歌单导入、多任务下载、自动降级、音频解密等功能。
            </p>
        </div>

        <div class="about-section">
            <h2 class="section-title">开源链接</h2>
            <n-ul class="link-list">
                <n-li>
                    <n-a href="https://github.com/lerdb/HotDownloader" target="_blank">
                        GitHub 仓库
                    </n-a>
                </n-li>
            </n-ul>
        </div>

        <div class="about-section">
            <h2 class="section-title">开放源代码许可</h2>
            <p class="license-text">
                本项目基于 <n-a href="https://www.apache.org/licenses/LICENSE-2.0" target="_blank">Apache License 2.0</n-a>
                开源。
            </p>
        </div>

        <div class="about-section">
            <h2 class="section-title">第三方组件</h2>
            <h3 class="sub-title">Rust</h3>
            <n-ul class="component-list">
                <n-li v-for="item in rustComponents" :key="item.name" class="component-item">
                    <span class="component-name">{{ item.name }}</span>
                    <span class="component-license">{{ item.license }}</span>
                </n-li>
            </n-ul>

            <h3 class="sub-title">Frontend</h3>
            <n-ul class="component-list">
                <n-li v-for="item in frontendComponents" :key="item.name" class="component-item">
                    <span class="component-name">{{ item.name }}</span>
                    <span class="component-license">{{ item.license }}</span>
                </n-li>
            </n-ul>
        </div>
    </div>
</template>

<script setup lang="ts">
import { NButton, NUl, NLi, NA } from 'naive-ui'
import { useRouter } from 'vue-router'
import { rustComponents, frontendComponents } from '../data/licenses'

const version = import.meta.env.VITE_APP_VERSION

const router = useRouter()

// 处理返回按钮点击逻辑，确保用户能正确回到上一页
function goBack() {
    // 若存在历史记录则直接后退，否则回退到设置页
    if (window.history.length > 1) {
        router.back()
    } else {
        router.push('/settings')
    }
}
</script>

<style scoped>
.about-view {
    max-width: 800px;
    min-width: 0;
    margin: 0 auto;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 16px;
}

.about-view > .n-button {
    align-self: flex-start;
}

.about-card {
    /* 使用全局定义的侧边栏背景色，自动适配深色模式 */
    background-color: var(--bg-sidebar);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    padding: 24px;
    text-align: center;
}

.app-title {
    font-size: 24px;
    font-weight: 700;
    margin-bottom: 4px;
    color: var(--color-text);
}

.app-version {
    font-size: 14px;
    color: var(--color-text-secondary);
    margin-bottom: 16px;
}

.app-description {
    font-size: 14px;
    color: var(--color-text-secondary);
    line-height: 1.6;
}

.about-section {
    background-color: var(--bg-sidebar);
    border: 1px solid var(--border-color);
    border-radius: 8px;
    padding: 16px 20px;
}

.section-title {
    font-size: 16px;
    font-weight: 600;
    margin-bottom: 12px;
    color: var(--color-text);
}

.sub-title {
    font-size: 14px;
    font-weight: 500;
    margin: 16px 0 8px;
    color: var(--color-text);
}

.link-list,
.component-list {
    list-style: none;
    padding: 0;
    margin: 0;
}

.component-item {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    flex-wrap: wrap;
    gap: 4px 16px;
    padding: 8px 0;
    border-bottom: 1px solid var(--border-color);
}

.component-item:last-child {
    border-bottom: none;
}

.component-name {
    overflow-wrap: anywhere;
    font-size: 14px;
    color: var(--color-text);
}

.component-license {
    overflow-wrap: anywhere;
    font-size: 13px;
    color: var(--color-text-secondary);
}

.license-text {
    font-size: 14px;
    color: var(--color-text-secondary);
    line-height: 1.6;
}

@media (max-width: 767px) {
    .about-card,
    .about-section {
        padding: 16px 12px;
    }
}
</style>
