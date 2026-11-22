<template>
    <div class="about-view">
        <n-button
            round
            secondary
            size="small"
            class="back-btn"
            @click="goBack"
        >
            <template #icon>
                <AppIcon name="back" :size="16" />
            </template>
            返回
        </n-button>

        <div class="about-card">
            <div class="about-badge" aria-hidden="true">
                <AppIcon name="music" :size="28" />
            </div>
            <h1 class="app-title">HotDownloader</h1>
            <div class="app-version">版本 {{ version }}</div>
            <p class="app-description">
                基于共享 Rust 下载核心和 Vue 3 的音乐下载工具，支持 Tauri
                桌面端、Android 端与 Docker/Web
                部署，提供搜索、歌单导入、多任务下载、自动降级、音频解密等功能。
            </p>
        </div>

        <div class="about-section">
            <h2 class="section-title">开源链接</h2>
            <n-ul class="link-list">
                <n-li>
                    <n-a
                        href="https://github.com/lerdb/HotDownloader"
                        target="_blank"
                    >
                        GitHub 仓库
                    </n-a>
                </n-li>
            </n-ul>
        </div>

        <div class="about-section">
            <h2 class="section-title">开放源代码许可</h2>
            <p class="license-text">
                本项目基于
                <n-a
                    href="https://www.apache.org/licenses/LICENSE-2.0"
                    target="_blank"
                    >Apache License 2.0</n-a
                >
                开源。
            </p>
        </div>

        <div class="about-section">
            <h2 class="section-title">第三方组件</h2>

            <h3 class="sub-title">Rust ({{ rustComponents.length }})</h3>
            <n-ul class="component-list">
                <n-li
                    v-for="item in rustComponents"
                    :key="item.name"
                    class="component-item component-item--clickable"
                    @click="openLicense(item)"
                >
                    <span class="component-name">{{ item.name }}</span>
                    <span class="component-license">{{ item.license }}</span>
                </n-li>
            </n-ul>

            <h3 class="sub-title">
                Frontend ({{ frontendComponents.length }})
            </h3>
            <n-ul class="component-list">
                <n-li
                    v-for="item in frontendComponents"
                    :key="item.name"
                    class="component-item component-item--clickable"
                    @click="openLicense(item)"
                >
                    <span class="component-name">{{ item.name }}</span>
                    <span class="component-license">{{ item.license }}</span>
                </n-li>
            </n-ul>
        </div>

        <n-modal
            v-model:show="showModal"
            preset="card"
            :title="modalTitle"
            style="width: min(720px, 92vw); max-height: 80vh"
            :bordered="false"
        >
            <n-scrollbar style="max-height: 60vh">
                <pre class="license-fulltext">{{ modalText }}</pre>
            </n-scrollbar>
        </n-modal>
    </div>
</template>

<script setup lang="ts">
import { ref, shallowRef } from 'vue'
import { NButton, NUl, NLi, NA, NModal, NScrollbar } from 'naive-ui'
import { useRouter } from 'vue-router'
import AppIcon from '../components/common/AppIcon.vue'
import {
    rustComponents as rustComponentData,
    frontendComponents as frontendComponentData,
    licenseTexts,
    type ComponentInfo,
} from '../data/licenses'

const rustComponents = shallowRef(rustComponentData)
const frontendComponents = shallowRef(frontendComponentData)

const version = import.meta.env.VITE_APP_VERSION

const router = useRouter()

const showModal = ref(false)
const modalTitle = ref('')
const modalText = ref('')

function goBack() {
    if (window.history.length > 1) {
        router.back()
    } else {
        router.push('/settings')
    }
}

function openLicense(item: ComponentInfo) {
    const ids = new Set<string>()
    const tokens = item.license
        .replace(/[()]/g, ' ')
        .split(/\s+/)
        .filter(Boolean)
    for (let i = 0; i < tokens.length; i++) {
        const t = tokens[i]
        if (t === 'OR' || t === 'AND') continue
        if (t === 'WITH') {
            i++
            continue
        }
        ids.add(t)
    }

    const parts: string[] = []
    for (const id of ids) {
        const found = licenseTexts.find((l) => l.id === id)
        if (found)
            parts.push(`── ${found.name} (${found.id}) ──\n\n${found.text}`)
    }

    modalTitle.value = `${item.name} @ ${item.version}`
    modalText.value =
        parts.length > 0 ? parts.join('\n\n') : '未找到许可证全文。'
    showModal.value = true
}
</script>

<style scoped>
.about-view {
    width: 100%;
    max-width: 820px;
    min-width: 0;
    margin: 0 auto;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 16px;
}

.back-btn {
    align-self: flex-start;
}

.about-card {
    background-color: var(--bg-sidebar);
    border: 1px solid var(--border-color);
    border-radius: 20px;
    padding: 28px 24px;
    text-align: center;
    box-shadow: var(--shadow-sm);
    display: flex;
    flex-direction: column;
    align-items: center;
}

.about-badge {
    width: 56px;
    height: 56px;
    border-radius: 16px;
    background: linear-gradient(
        135deg,
        var(--color-primary),
        var(--color-primary-hover)
    );
    color: #ffffff;
    display: grid;
    place-items: center;
    margin-bottom: 14px;
    box-shadow: 0 10px 24px var(--color-primary-soft);
}

:root[data-theme='dark'] .about-badge {
    color: #0b1210;
}

.app-title {
    font-size: 24px;
    font-weight: 700;
    margin-bottom: 6px;
    color: var(--color-text);
}

.app-version {
    display: inline-block;
    padding: 2px 12px;
    border-radius: 999px;
    background: var(--color-primary-soft);
    color: var(--color-primary);
    font-size: 12px;
    font-weight: 600;
    margin-bottom: 16px;
}

.app-description {
    max-width: 580px;
    font-size: 14px;
    color: var(--color-text-secondary);
    line-height: 1.65;
}

.about-section {
    background-color: var(--bg-sidebar);
    border: 1px solid var(--border-color);
    border-radius: 16px;
    padding: 18px 20px;
    box-shadow: var(--shadow-sm);
}

.section-title {
    font-size: 16px;
    font-weight: 650;
    margin-bottom: 12px;
    color: var(--color-text);
}

.sub-title {
    font-size: 14px;
    font-weight: 600;
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
    padding: 9px 8px;
    border-bottom: 1px solid var(--border-color);
}

.component-item:last-child {
    border-bottom: none;
}

.component-item--clickable {
    cursor: pointer;
    border-radius: 8px;
    transition: background-color 0.15s;
}

.component-item--clickable:hover {
    background-color: var(--bg-surface-subtle);
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

.license-fulltext {
    font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
    font-size: 12px;
    line-height: 1.5;
    white-space: pre-wrap;
    word-break: break-word;
    color: var(--color-text-secondary);
    margin: 0;
}

@media (max-width: 767px) {
    .about-card,
    .about-section {
        padding: 18px 14px;
    }
}
</style>
