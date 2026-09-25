<template>
  <n-config-provider :theme="theme" :theme-overrides="themeOverrides" class="app-root">
    <n-dialog-provider>
      <n-notification-provider>
        <NavLayout v-if="native || webSession.authorized" />
        <WebAccessGate v-else />
        <UpdateChecker v-if="native" :show-entry="false" show-modal />
      </n-notification-provider>
    </n-dialog-provider>
  </n-config-provider>
</template>

<script setup lang="ts">
import { onMounted } from 'vue'
import { NConfigProvider, NDialogProvider, NNotificationProvider } from 'naive-ui'
import NavLayout from './components/NavLayout.vue'
import UpdateChecker from './components/settings/UpdateChecker.vue'
import WebAccessGate from './components/WebAccessGate.vue'
import { isNativeRuntime } from './api/runtimeApi'
import { webSession } from './api/webClient'
import { useAppTheme } from './composables/useAppTheme'
import { useUpdateChecker } from './composables/useUpdateChecker'

const { theme, themeOverrides } = useAppTheme()
const { initializeUpdateChecker } = useUpdateChecker()
const native = isNativeRuntime()

onMounted(() => {
  if (native) {
    void initializeUpdateChecker()
  }
})
</script>

<style scoped>
.app-root {
  height: 100%;
  color: var(--color-text);
}
</style>
