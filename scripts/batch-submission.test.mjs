import assert from 'node:assert/strict'
import test from 'node:test'
import { createBatchSubmission } from '../src/utils/batchSubmission.ts'

test('stopping an in-flight batch waits for that request and submits no later songs', async () => {
    let progress
    const batch = createBatchSubmission((value) => {
        progress = value
    }, 0)
    const submitted = []
    let release
    const run = batch.run([1, 2, 3, 4], async (song) => {
        submitted.push(song)
        await new Promise((resolve) => {
            release = resolve
        })
    })
    const stopped = batch.stop()
    assert.equal(progress.stopping, true)
    assert.deepEqual(submitted, [1])
    release()
    await Promise.all([run, stopped])
    assert.deepEqual(submitted, [1])
    assert.deepEqual(progress, {
        running: false,
        stopping: false,
        processed: 1,
        total: 4,
    })
})

test('stopping interrupts the submission buffer and permits a new batch', async () => {
    let progress
    const batch = createBatchSubmission((value) => {
        progress = value
    }, 10000)
    const submitted = []
    const run = batch.run([1, 2], async (song) => {
        submitted.push(song)
    })
    await new Promise((resolve) => setTimeout(resolve, 5))
    await batch.stop()
    await run
    assert.deepEqual(submitted, [1])
    await batch.run([3], async (song) => {
        submitted.push(song)
    })
    assert.deepEqual(submitted, [1, 3])
    assert.equal(progress.running, false)
})

test('overlapping submissions are rejected and failed requests release the batch', async () => {
    const batch = createBatchSubmission(() => {}, 0)
    let reject
    const first = batch.run(
        [1, 2],
        () =>
            new Promise((_, fail) => {
                reject = fail
            }),
    )
    await assert.rejects(
        batch.run([3], async () => {}),
        /已有一批/,
    )
    reject(new Error('storage unavailable'))
    await assert.rejects(first, /storage unavailable/)
    await batch.run([4], async () => {})
})
