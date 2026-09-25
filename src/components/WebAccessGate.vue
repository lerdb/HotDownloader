<template>
    <div class="access-page">
        <n-card class="access-card" title="连接下载服务">
            <p>请输入服务部署时配置的访问令牌。令牌仅保存在当前浏览器会话中。</p>
            <n-input
                v-model:value="token"
                type="password"
                show-password-on="click"
                placeholder="访问令牌"
                @keyup.enter="submit"
            />
            <n-alert v-if="webSession.error" type="error" class="access-error">
                {{ webSession.error }}
            </n-alert>
            <n-button type="primary" :loading="webSession.checking" class="access-button" @click="submit">
                连接
            </n-button>
        </n-card>
    </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { NAlert, NButton, NCard, NInput } from 'naive-ui'
import { authorizeWeb, webSession } from '../api/webClient'

const token = ref('')

async function submit() {
    await authorizeWeb(token.value)
}
</script>

<style scoped>
.access-page {
    min-height: 100vh;
    display: grid;
    place-items: center;
    padding: 24px;
}

.access-card {
    width: min(440px, 100%);
}

.access-card p {
    margin: 0 0 18px;
    line-height: 1.6;
}

.access-error {
    margin-top: 12px;
}

.access-button {
    width: 100%;
    margin-top: 18px;
}
</style>
