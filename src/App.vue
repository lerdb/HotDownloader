<template>
  <n-config-provider :theme="theme" :theme-overrides="themeOverrides" class="app-root">
    <n-dialog-provider>
      <n-notification-provider>
        <NavLayout />
      </n-notification-provider>
    </n-dialog-provider>
  </n-config-provider>
</template>

<script setup lang="ts">
import { NConfigProvider, NDialogProvider, NNotificationProvider } from 'naive-ui'
import NavLayout from './components/NavLayout.vue'
import { useAppTheme } from './composables/useAppTheme'

const { theme, themeOverrides } = useAppTheme()
</script>

<style scoped>
.app-root {
  height: 100%;
  color: var(--color-text);
}
</style>
