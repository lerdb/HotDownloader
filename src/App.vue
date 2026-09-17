<template>
  <n-config-provider :theme="theme" :theme-overrides="themeOverrides" class="app-root">
    <n-dialog-provider>
      <n-notification-provider>
        <NavLayout />
        <UpdateChecker :show-entry="false" show-modal />
      </n-notification-provider>
    </n-dialog-provider>
  </n-config-provider>
</template>

<script setup lang="ts">
import { onMounted } from 'vue'
import { NConfigProvider, NDialogProvider, NNotificationProvider } from 'naive-ui'
import NavLayout from './components/NavLayout.vue'
import UpdateChecker from './components/settings/UpdateChecker.vue'
import { useAppTheme } from './composables/useAppTheme'
import { useUpdateChecker } from './composables/useUpdateChecker'

const { theme, themeOverrides } = useAppTheme()
const { initializeUpdateChecker } = useUpdateChecker()

onMounted(() => {
  void initializeUpdateChecker()
})
</script>

<style scoped>
.app-root {
  height: 100%;
  color: var(--color-text);
}
</style>
