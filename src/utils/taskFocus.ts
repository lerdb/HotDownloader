import type { TaskRecord } from '../types'

export function taskFocusId(path: string, value: unknown): string {
    return path === '/task' && typeof value === 'string' ? value : ''
}

export function filterTasks(
    tasks: TaskRecord[],
    tab: string,
    focusedId: string,
): TaskRecord[] {
    if (focusedId) return tasks.filter((task) => task.id === focusedId)
    return tasks.filter((task) => tab === 'all' || task.status === tab)
}
