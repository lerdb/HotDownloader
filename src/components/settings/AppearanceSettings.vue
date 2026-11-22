<template>
    <section class="appearance-settings" aria-labelledby="appearance-title">
        <header class="appearance-header">
            <div class="title-row">
                <AppIcon name="palette" :size="18" class="title-icon" />
                <h2 id="appearance-title">外观与主题</h2>
            </div>
            <p>参考 SimpMusic 风格，自定义明暗外观与强调色主题</p>
        </header>

        <div class="appearance-block">
            <span class="block-label">外观模式</span>
            <div class="mode-pills" role="radiogroup" aria-label="外观模式">
                <button
                    v-for="mode in modes"
                    :key="mode.value"
                    type="button"
                    role="radio"
                    :aria-checked="themeMode === mode.value"
                    class="mode-pill"
                    :class="{ 'is-active': themeMode === mode.value }"
                    @click="setThemeMode(mode.value)"
                >
                    <AppIcon :name="mode.icon" :size="15" />
                    <span>{{ mode.label }}</span>
                </button>
            </div>
        </div>

        <div class="appearance-block">
            <span class="block-label">主题强调色</span>
            <div class="preset-grid" role="radiogroup" aria-label="主题强调色">
                <button
                    v-for="preset in ACCENT_PRESETS"
                    :key="preset.key"
                    type="button"
                    role="radio"
                    :aria-checked="accentPreset === preset.key"
                    class="preset-chip"
                    :class="{ 'is-active': accentPreset === preset.key }"
                    @click="setAccentPreset(preset.key)"
                >
                    <span
                        class="preset-swatch"
                        :style="{
                            backgroundColor: isDark
                                ? preset.darkPrimary
                                : preset.lightPrimary,
                        }"
                    >
                        <AppIcon
                            v-if="accentPreset === preset.key"
                            name="check"
                            :size="12"
                            class="swatch-check"
                        />
                    </span>
                    <span class="preset-name">{{ preset.label }}</span>
                </button>
            </div>
        </div>
    </section>
</template>

<script setup lang="ts">
import { ACCENT_PRESETS, type ThemeMode } from '../../config/theme'
import { useAppTheme } from '../../composables/useAppTheme'
import AppIcon, { type AppIconName } from '../common/AppIcon.vue'

const { isDark, themeMode, accentPreset, setThemeMode, setAccentPreset } =
    useAppTheme()

const modes: { value: ThemeMode; label: string; icon: AppIconName }[] = [
    { value: 'system', label: '跟随系统', icon: 'monitor' },
    { value: 'dark', label: '深色模式', icon: 'moon' },
    { value: 'light', label: '浅色模式', icon: 'sun' },
]
</script>

<style scoped>
.appearance-settings {
    padding: 22px;
    background: var(--bg-sidebar);
    border: 1px solid var(--border-color);
    border-radius: 16px;
    margin-bottom: 18px;
    box-shadow: var(--shadow-sm);
}

.appearance-header {
    margin-bottom: 18px;
}

.title-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 4px;
}

.title-icon {
    color: var(--color-primary);
}

.appearance-header h2 {
    font-size: 17px;
    font-weight: 650;
    margin: 0;
}

.appearance-header p {
    font-size: 13px;
    color: var(--color-text-secondary);
}

.appearance-block + .appearance-block {
    margin-top: 16px;
}

.block-label {
    display: block;
    font-size: 13px;
    font-weight: 500;
    color: var(--color-text-secondary);
    margin-bottom: 10px;
}

.mode-pills {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
}

.mode-pill {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    min-height: 38px;
    padding: 7px 14px;
    border-radius: 999px;
    border: 1px solid var(--border-color);
    background: var(--bg-surface-subtle);
    color: var(--color-text-secondary);
    font: inherit;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    transition:
        background-color 0.18s ease,
        color 0.18s ease,
        border-color 0.18s ease;
}

.mode-pill:hover {
    color: var(--color-text);
    border-color: var(--border-color-strong);
}

.mode-pill.is-active {
    background: var(--color-primary-soft);
    color: var(--color-primary);
    border-color: var(--color-primary);
    font-weight: 600;
}

.preset-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(150px, 1fr));
    gap: 8px;
}

.preset-chip {
    display: flex;
    align-items: center;
    gap: 9px;
    min-height: 40px;
    padding: 8px 12px;
    border-radius: 12px;
    border: 1px solid var(--border-color);
    background: var(--bg-surface-subtle);
    color: var(--color-text);
    font: inherit;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    text-align: left;
    transition:
        border-color 0.18s ease,
        background-color 0.18s ease,
        transform 0.15s ease;
}

.preset-chip:hover {
    border-color: var(--border-color-strong);
    transform: translateY(-1px);
}

.preset-chip.is-active {
    border-color: var(--color-primary);
    background: var(--color-primary-soft);
    font-weight: 600;
}

.preset-swatch {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    flex-shrink: 0;
    display: grid;
    place-items: center;
    color: #ffffff;
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.22);
}

:root[data-theme='dark'] .swatch-check {
    color: #0b1210;
}

.preset-name {
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
}

@media (max-width: 600px) {
    .appearance-settings {
        padding: 16px 14px;
    }

    .preset-grid {
        grid-template-columns: repeat(2, minmax(0, 1fr));
    }
}
</style>
