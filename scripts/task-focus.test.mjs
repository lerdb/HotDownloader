import assert from 'node:assert/strict'
import test from 'node:test'
import { filterTasks, taskFocusId } from '../src/utils/taskFocus.ts'

test('monitor task link finds a task beyond page one regardless of old status tab', () => {
    const tasks = Array.from({ length: 125 }, (_, i) => ({
        id: `task-${i}`,
        status: i === 124 ? 'completed' : 'waiting',
    }))
    const focused = filterTasks(
        tasks,
        'error',
        taskFocusId('/task', 'task-124'),
    )
    assert.deepEqual(focused, [tasks[124]])
    assert.equal(focused.slice(0, 50).length, 1)
})

test('focus follows status changes and reports a removed task as missing', () => {
    const target = { id: 'chosen', status: 'waiting' }
    assert.equal(filterTasks([target], 'error', 'chosen')[0], target)
    target.status = 'completed'
    assert.equal(
        filterTasks([target], 'error', 'chosen')[0].status,
        'completed',
    )
    assert.deepEqual(filterTasks([], 'all', 'chosen'), [])
    assert.deepEqual(
        filterTasks([{ id: 'another', status: 'waiting' }], 'all', 'chosen'),
        [],
    )
})

test('leaving task view or clearing focus restores normal tab filtering', () => {
    const tasks = [
        { id: 'a', status: 'waiting' },
        { id: 'b', status: 'error' },
    ]
    assert.equal(taskFocusId('/playlist/monitors', 'a'), '')
    assert.equal(taskFocusId('/task', ['a', 'b']), '')
    assert.equal(taskFocusId('/task', undefined), '')
    assert.deepEqual(filterTasks(tasks, 'error', ''), [tasks[1]])
    assert.deepEqual(filterTasks(tasks, 'all', ''), tasks)
})
