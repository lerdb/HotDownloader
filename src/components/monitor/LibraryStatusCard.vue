<template>
    <n-card title="本地音乐库" size="small">
        <template #header-extra
            ><n-button
                :loading="scanning || library?.scanning"
                :disabled="busy"
                @click="emit('scan')"
                >立即扫描</n-button
            ></template
        >
        <template v-if="library">
            <p>
                已索引 {{ library.fileCount }} 个文件 · 最近完整扫描：{{
                    time(library.lastSuccess)
                }}
            </p>
            <n-alert v-if="library.error" type="warning" title="本轮补齐已跳过"
                >{{ library.error }}。保留上次完整索引。</n-alert
            >
            <p v-if="library.unresolvedCount" class="muted">
                {{ library.unresolvedCount }}
                个文件信息不完整，无法可靠匹配的歌曲会进入待确认。
            </p>
            <n-button @click="emit('issues')">查看异常文件</n-button>
            <p class="muted">
                自动完整扫描最多每 5
                分钟一次；修复标签或文件后可立即扫描，再逐曲刷新候选。
            </p>
            <div
                v-for="root in library.roots"
                :key="root.path"
                class="root-row"
            >
                <code>{{ root.path }}</code>
                <span class="muted"
                    >模板：{{ root.template || '仅使用音频标签' }} ·
                    歌手分隔符：{{ root.artistSeparator }}</span
                >
            </div>
            <p class="muted">
                目录由服务端环境变量配置；下载目录自动扫描，其他 NAS
                目录请只读挂载。
            </p>
            <div
                v-for="directory in library.directories"
                :key="directory.path"
                class="directory-status"
            >
                <code>{{ directory.path }}</code>
                <n-tag
                    :type="
                        directory.state === 'unavailable'
                            ? 'error'
                            : directory.state === 'warning'
                              ? 'warning'
                              : directory.state === 'healthy'
                                ? 'success'
                                : 'default'
                    "
                >
                    {{
                        {
                            unknown: '尚未检查',
                            healthy: '可读取',
                            warning: '文件信息需检查',
                            unavailable: '不可访问',
                        }[directory.state]
                    }}
                </n-tag>
                <p>
                    已索引 {{ directory.fileCount }} 个文件 · 索引时间：{{
                        time(directory.indexedAt)
                    }}
                </p>
                <p class="muted">
                    最近检查：{{ time(directory.checkedAt)
                    }}<template v-if="directory.observedCount !== null">
                        · 本次读取 {{ directory.observedCount }} 个文件 ·
                        信息异常 {{ directory.warningCount }} 个</template
                    >
                </p>
                <p v-if="directory.error">{{ directory.error }}</p>
            </div>
            <p v-if="library.error" class="muted">
                本轮未更新索引；各目录的“已索引”数量仍来自上次完整扫描。
            </p>
        </template>
    </n-card>
</template>

<script setup lang="ts">
import { NAlert, NButton, NCard, NTag } from 'naive-ui'
import type { LibraryStatus } from '../../api/monitorApi'
import { time } from './presentation'
defineProps<{ library?: LibraryStatus; scanning: boolean; busy: boolean }>()
const emit = defineEmits<{ scan: []; issues: [] }>()
</script>

<style scoped src="./monitor-shared.css"></style>
<style scoped>
.root-row {
    display: flex;
    flex-direction: column;
    gap: 8px;
    margin: 12px 0;
}
.directory-status {
    margin-top: 12px;
    padding: 12px;
    border: 1px solid var(--border-color);
    border-radius: 8px;
}
.directory-status code {
    margin-right: 12px;
}
</style>
