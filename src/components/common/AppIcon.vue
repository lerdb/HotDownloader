<template>
    <svg
        class="app-icon"
        :width="size"
        :height="size"
        viewBox="0 0 24 24"
        fill="none"
        xmlns="http://www.w3.org/2000/svg"
        aria-hidden="true"
    >
        <template v-if="name === 'search'">
            <circle
                cx="11"
                cy="11"
                r="7"
                stroke="currentColor"
                stroke-width="2"
            />
            <path
                d="M16.5 16.5L21 21"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
            />
        </template>

        <template v-else-if="name === 'playlist'">
            <path
                d="M4 6H16M4 11H12M4 16H10"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
            />
            <path
                d="M16 17V9L21 8V15"
                stroke="currentColor"
                stroke-width="1.9"
                stroke-linecap="round"
                stroke-linejoin="round"
            />
            <circle cx="14" cy="17.5" r="2.2" fill="currentColor" />
            <circle cx="19" cy="15.5" r="2.2" fill="currentColor" />
        </template>

        <template v-else-if="name === 'task'">
            <path
                d="M12 3V14M12 14L7.5 9.5M12 14L16.5 9.5"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
            />
            <path
                d="M4 16.5V18.5C4 19.6 4.9 20.5 6 20.5H18C19.1 20.5 20 19.6 20 18.5V16.5"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
            />
        </template>

        <template v-else-if="name === 'settings'">
            <circle
                cx="12"
                cy="12"
                r="3"
                stroke="currentColor"
                stroke-width="2"
            />
            <path
                d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 1 1-4 0v-.09a1.65 1.65 0 0 0-1-1.51 1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 1 1 0-4h.09a1.65 1.65 0 0 0 1.51-1 1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33H9a1.65 1.65 0 0 0 1-1.51V3a2 2 0 1 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 1 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82V9c.26.6.79 1 1.51 1H21a2 2 0 1 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"
                stroke="currentColor"
                stroke-width="1.8"
                stroke-linecap="round"
                stroke-linejoin="round"
            />
        </template>

        <template v-else-if="name === 'play'">
            <path d="M7.5 5.2L19 12L7.5 18.8V5.2Z" fill="currentColor" />
        </template>

        <template v-else-if="name === 'pause'">
            <rect
                x="6.5"
                y="5"
                width="4"
                height="14"
                rx="1.5"
                fill="currentColor"
            />
            <rect
                x="13.5"
                y="5"
                width="4"
                height="14"
                rx="1.5"
                fill="currentColor"
            />
        </template>

        <template v-else-if="name === 'close'">
            <path
                d="M6.5 6.5L17.5 17.5M17.5 6.5L6.5 17.5"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
            />
        </template>

        <template v-else-if="name === 'download'">
            <path
                d="M12 4V14.5M12 14.5L8 10.5M12 14.5L16 10.5"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
            />
            <path
                d="M5 18.5H19"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
            />
        </template>

        <template v-else-if="name === 'music'">
            <path
                d="M9 18V6L20 4V16"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
            />
            <circle cx="6.5" cy="18" r="2.5" fill="currentColor" />
            <circle cx="17.5" cy="16" r="2.5" fill="currentColor" />
        </template>

        <template v-else-if="name === 'disc'">
            <circle
                cx="12"
                cy="12"
                r="9"
                stroke="currentColor"
                stroke-width="1.9"
            />
            <circle
                cx="12"
                cy="12"
                r="2.5"
                stroke="currentColor"
                stroke-width="1.9"
            />
        </template>

        <template v-else-if="name === 'artist'">
            <circle
                cx="12"
                cy="8"
                r="4"
                stroke="currentColor"
                stroke-width="2"
            />
            <path
                d="M4.5 20C5.3 16.5 8.3 14.5 12 14.5C15.7 14.5 18.7 16.5 19.5 20"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
            />
        </template>

        <template v-else-if="name === 'history'">
            <circle
                cx="12"
                cy="12"
                r="8.5"
                stroke="currentColor"
                stroke-width="1.9"
            />
            <path
                d="M12 7.5V12.2L15.2 14"
                stroke="currentColor"
                stroke-width="1.9"
                stroke-linecap="round"
                stroke-linejoin="round"
            />
        </template>

        <template v-else-if="name === 'flame'">
            <path
                d="M12.5 2.5C12.5 2.5 17 6.5 17 11.2C17 14.5 14.8 17 12 17C13.2 15.8 13.7 14.4 13.5 12.8C13.2 11 11.8 9.6 11.8 9.6C11.8 9.6 11.5 12 9.8 13.2C8.6 14 7 15.2 7 16.8C7 19.1 9.2 21 12 21C16 21 19 18 19 13.8C19 8.4 14.8 4.2 12.5 2.5Z"
                fill="currentColor"
                opacity="0.92"
            />
        </template>

        <template v-else-if="name === 'back'">
            <path
                d="M15 6L9 12L15 18"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
            />
        </template>

        <template v-else-if="name === 'volume'">
            <path
                d="M5 9.5H8.5L13.5 5.5V18.5L8.5 14.5H5V9.5Z"
                fill="currentColor"
            />
            <path
                d="M16.8 8.8C18 10 18.6 11 18.6 12C18.6 13 18 14 16.8 15.2"
                stroke="currentColor"
                stroke-width="1.9"
                stroke-linecap="round"
            />
        </template>

        <template v-else-if="name === 'volume-mute'">
            <path
                d="M5 9.5H8.5L13.5 5.5V18.5L8.5 14.5H5V9.5Z"
                fill="currentColor"
            />
            <path
                d="M16.5 9.5L20.5 14.5M20.5 9.5L16.5 14.5"
                stroke="currentColor"
                stroke-width="1.9"
                stroke-linecap="round"
            />
        </template>

        <template v-else-if="name === 'expand'">
            <path
                d="M6 15L12 9L18 15"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
            />
        </template>

        <template v-else-if="name === 'collapse'">
            <path
                d="M6 9L12 15L18 9"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
            />
        </template>

        <template v-else-if="name === 'sun'">
            <circle
                cx="12"
                cy="12"
                r="4"
                stroke="currentColor"
                stroke-width="2"
            />
            <path
                d="M12 2.5V4.5M12 19.5V21.5M4.5 12H2.5M21.5 12H19.5M5.6 5.6L7 7M17 17L18.4 18.4M18.4 5.6L17 7M7 17L5.6 18.4"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
            />
        </template>

        <template v-else-if="name === 'moon'">
            <path
                d="M19.5 14.5A7.5 7.5 0 0 1 9.5 4.5A7.5 7.5 0 1 0 19.5 14.5Z"
                stroke="currentColor"
                stroke-width="2"
                stroke-linejoin="round"
            />
        </template>

        <template v-else-if="name === 'monitor'">
            <rect
                x="3"
                y="4"
                width="18"
                height="13"
                rx="2.5"
                stroke="currentColor"
                stroke-width="2"
            />
            <path
                d="M8.5 20H15.5M12 17V20"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
            />
        </template>

        <template v-else-if="name === 'palette'">
            <path
                d="M12 3C7 3 3 6.8 3 11.5C3 14.5 5.1 16.5 7.5 16.5C8.6 16.5 9.2 17.2 9.2 18.2C9.2 19.8 10.5 21 12.2 21C17.2 21 21 16.9 21 12C21 7 17 3 12 3Z"
                stroke="currentColor"
                stroke-width="1.9"
            />
            <circle cx="8" cy="10.5" r="1.3" fill="currentColor" />
            <circle cx="11.5" cy="7.5" r="1.3" fill="currentColor" />
            <circle cx="16" cy="9.5" r="1.3" fill="currentColor" />
        </template>

        <template v-else-if="name === 'radar'">
            <circle cx="12" cy="12" r="2" fill="currentColor" />
            <circle
                cx="12"
                cy="12"
                r="5.5"
                stroke="currentColor"
                stroke-width="1.8"
            />
            <circle
                cx="12"
                cy="12"
                r="9"
                stroke="currentColor"
                stroke-width="1.8"
                stroke-dasharray="3 3"
            />
        </template>

        <template v-else-if="name === 'chevron-right'">
            <path
                d="M9 6L15 12L9 18"
                stroke="currentColor"
                stroke-width="2"
                stroke-linecap="round"
                stroke-linejoin="round"
            />
        </template>

        <template v-else-if="name === 'check'">
            <path
                d="M5.5 12.5L10 17L18.5 7.5"
                stroke="currentColor"
                stroke-width="2.2"
                stroke-linecap="round"
                stroke-linejoin="round"
            />
        </template>

        <template v-else-if="name === 'sparkles'">
            <path
                d="M12 3L13.8 8.2L19 10L13.8 11.8L12 17L10.2 11.8L5 10L10.2 8.2L12 3Z"
                fill="currentColor"
            />
            <path
                d="M18.5 15.5L19.3 17.7L21.5 18.5L19.3 19.3L18.5 21.5L17.7 19.3L15.5 18.5L17.7 17.7L18.5 15.5Z"
                fill="currentColor"
            />
        </template>
    </svg>
</template>

<script setup lang="ts">
export type AppIconName =
    | 'search'
    | 'playlist'
    | 'task'
    | 'settings'
    | 'play'
    | 'pause'
    | 'close'
    | 'download'
    | 'music'
    | 'disc'
    | 'artist'
    | 'history'
    | 'flame'
    | 'back'
    | 'volume'
    | 'volume-mute'
    | 'expand'
    | 'collapse'
    | 'sun'
    | 'moon'
    | 'monitor'
    | 'palette'
    | 'radar'
    | 'chevron-right'
    | 'check'
    | 'sparkles'

withDefaults(
    defineProps<{
        name: AppIconName
        size?: number
    }>(),
    {
        size: 18,
    },
)
</script>

<style scoped>
.app-icon {
    display: inline-block;
    flex-shrink: 0;
    vertical-align: middle;
}
</style>
