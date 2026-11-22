<template>
    <div class="access-page">
        <div class="access-glow" aria-hidden="true" />
        <div class="access-card">
            <div class="access-brand">
                <div class="brand-badge" aria-hidden="true">
                    <AppIcon name="music" :size="24" />
                </div>
                <h1>HotDownloader</h1>
                <p class="brand-desc">连接下载服务以开始管理音乐与任务</p>
            </div>

            <template v-if="webSession.mode === 'password'">
                <p class="prompt-text">请输入服务部署时配置的用户名和密码。</p>
                <form @submit.prevent="submit" class="access-form">
                    <n-input
                        v-model:value="username"
                        name="username"
                        size="large"
                        placeholder="用户名"
                        autocomplete="username"
                    />
                    <n-input
                        v-model:value="password"
                        type="password"
                        show-password-on="click"
                        size="large"
                        name="password"
                        placeholder="密码"
                        autocomplete="current-password"
                    />
                    <n-button
                        type="primary"
                        size="large"
                        round
                        :loading="webSession.checking"
                        class="access-button"
                        attr-type="submit"
                        >登录</n-button
                    >
                </form>
            </template>
            <template v-else-if="webSession.mode === 'token'">
                <p class="prompt-text">请输入服务部署时配置的访问令牌。</p>
                <form @submit.prevent="submit" class="access-form">
                    <n-input
                        v-model:value="token"
                        type="password"
                        show-password-on="click"
                        size="large"
                        placeholder="访问令牌"
                        autocomplete="off"
                    />
                    <n-button
                        type="primary"
                        size="large"
                        round
                        :loading="webSession.checking"
                        class="access-button"
                        attr-type="submit"
                        >连接服务</n-button
                    >
                </form>
            </template>
            <template v-else>
                <p class="prompt-text">正在连接下载服务…</p>
                <n-button
                    v-if="webSession.error"
                    round
                    type="primary"
                    :loading="webSession.checking"
                    class="access-button"
                    @click="authorizeWeb()"
                >
                    重试连接
                </n-button>
            </template>
            <n-alert v-if="webSession.error" type="error" class="access-error">
                {{ webSession.error }}
            </n-alert>
        </div>
    </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { NAlert, NButton, NInput } from 'naive-ui'
import AppIcon from './common/AppIcon.vue'
import {
    authorizeWeb,
    authorizeWebWithPassword,
    webSession,
} from '../api/webClient'

const token = ref('')
const username = ref('')
const password = ref('')

async function submit() {
    if (webSession.mode === 'password') {
        await authorizeWebWithPassword(username.value, password.value)
        password.value = ''
    } else {
        await authorizeWeb(token.value)
    }
}
</script>

<style scoped>
.access-page {
    position: relative;
    min-height: 100vh;
    min-height: 100dvh;
    display: grid;
    place-items: center;
    padding: 24px;
    background: var(--bg-body);
    overflow: hidden;
}

.access-glow {
    position: absolute;
    width: 480px;
    height: 480px;
    border-radius: 50%;
    background: radial-gradient(
        circle,
        var(--color-primary-soft) 0%,
        transparent 70%
    );
    filter: blur(40px);
    pointer-events: none;
}

.access-card {
    position: relative;
    z-index: 1;
    width: min(420px, 100%);
    padding: 28px 26px;
    border-radius: 22px;
    background: var(--bg-glass-strong);
    backdrop-filter: blur(22px) saturate(160%);
    -webkit-backdrop-filter: blur(22px) saturate(160%);
    border: 1px solid var(--border-color);
    box-shadow: var(--shadow-lg);
}

.access-brand {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    margin-bottom: 22px;
}

.brand-badge {
    width: 52px;
    height: 52px;
    border-radius: 16px;
    background: linear-gradient(
        135deg,
        var(--color-primary),
        var(--color-primary-hover)
    );
    color: #ffffff;
    display: grid;
    place-items: center;
    margin-bottom: 12px;
    box-shadow: 0 10px 24px var(--color-primary-soft);
}

:root[data-theme='dark'] .brand-badge {
    color: #0b1210;
}

.access-brand h1 {
    font-size: 22px;
    font-weight: 700;
    margin: 0 0 4px;
}

.brand-desc {
    font-size: 13px;
    color: var(--color-text-secondary);
    margin: 0;
}

.prompt-text {
    margin: 0 0 14px;
    font-size: 13px;
    color: var(--color-text-secondary);
    line-height: 1.6;
    text-align: center;
}

.access-form {
    display: flex;
    flex-direction: column;
    gap: 12px;
}

.access-error {
    margin-top: 14px;
    border-radius: 12px;
}

.access-button {
    width: 100%;
    margin-top: 6px;
}
</style>
