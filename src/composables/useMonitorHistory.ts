import { ref } from 'vue'
import * as api from '../api/monitorApi'
import { monitorErrorMessage as message } from '../components/monitor/presentation'

export function useMonitorHistory() {
    const showHistory = ref(false),
        historyLoading = ref(false),
        historyError = ref(''),
        historyId = ref(''),
        historyName = ref('')
    const history = ref<api.CheckRecord[]>([]),
        historyPage = ref(1)
    async function openHistory(m: api.Monitor) {
        historyId.value = m.id
        historyName.value = m.name
        history.value = []
        historyPage.value = 1
        showHistory.value = true
        await loadHistory()
    }
    async function loadHistory() {
        const id = historyId.value
        historyLoading.value = true
        historyError.value = ''
        try {
            const result = await api.getMonitorHistory(id)
            if (historyId.value === id) history.value = result
        } catch (e) {
            if (historyId.value === id) historyError.value = message(e)
        } finally {
            if (historyId.value === id) historyLoading.value = false
        }
    }

    return {
        showHistory,
        historyLoading,
        historyError,
        historyName,
        history,
        historyPage,
        openHistory,
        loadHistory,
    }
}
