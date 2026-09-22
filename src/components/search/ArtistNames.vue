<template>
    <span>
        <template v-if="artists?.length">
            <template v-for="(artist, index) in artists" :key="index">
                <template v-if="index">{{ settingsStore.settings.artistSeparator }}</template>
                <n-button v-if="getMusicEntityId(platform, artist.id, artist.mid)" text size="small"
                    @click.stop="$emit('click-artist', platform, artist)">
                    {{ artist.name }}
                </n-button>
                <span v-else>{{ artist.name }}</span>
            </template>
        </template>
        <template v-else>{{ fallback }}</template>
    </span>
</template>

<script setup lang="ts">
import type { ArtistReference } from '../../types'
import { NButton } from 'naive-ui'
import { useSettingsStore } from '../../stores/settingsStore'
import { getMusicEntityId } from '../../utils/music'

defineProps<{
    platform: string
    artists?: ArtistReference[]
    fallback: string
}>()

defineEmits<{
    (e: 'click-artist', platform: string, artist: ArtistReference): void
}>()

const settingsStore = useSettingsStore()
</script>

<style scoped>
.n-button {
    font: inherit;
    vertical-align: baseline;
}
</style>
