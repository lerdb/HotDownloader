import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import path from 'node:path'
import test from 'node:test'
import vm from 'node:vm'
import ts from 'typescript'
import * as vue from 'vue'

const workspace = path.resolve(import.meta.dirname, '..')
const plain = (value) => JSON.parse(JSON.stringify(value))
const settle = () => new Promise((resolve) => setImmediate(resolve))
const entry = (mid, state = 'pending_confirmation') => ({
    song: { mid, title: mid, artist: '虚构歌手' },
    state,
})
const monitor = (id = 'monitor-a') => ({
    id,
    name: id,
    source: 'public',
    playlistId: '123456',
    dirid: '',
    quality: 'flac',
    intervalMinutes: 60,
    enabled: false,
})

function harness(overrides = {}) {
    const hooks = {}
    const timers = new Map()
    let timerId = 0
    const route = vue.reactive({
        path: '/playlist/monitors',
        fullPath: '/playlist/monitors',
        query: {},
    })
    const router = {
        replace(next) {
            Object.assign(route, next, {
                fullPath: next.path,
                query: next.query ?? {},
            })
            return Promise.resolve()
        },
    }
    const api = {
        getLibrary: async () => ({ fileCount: 1 }),
        getMonitors: async () => ({
            monitors: [monitor()],
            running: false,
            persistenceFailed: false,
        }),
        getMonitorSongs: async () => ({
            items: [entry('song-a'), entry('song-b')],
            total: 2,
            page: 1,
            pageSize: 30,
        }),
        getMonitorHistory: async () => [],
        samePlaylist: (a, b) =>
            a.source === b.source && a.playlistId === b.playlistId,
        ...overrides,
    }
    const modules = new Map()
    function load(relativePath) {
        const file = path.resolve(workspace, relativePath)
        if (modules.has(file)) return modules.get(file)
        const exports = {}
        modules.set(file, exports)
        const { outputText } = ts.transpileModule(readFileSync(file, 'utf8'), {
            compilerOptions: {
                module: ts.ModuleKind.CommonJS,
                target: ts.ScriptTarget.ES2022,
            },
        })
        vm.runInNewContext(
            outputText,
            {
                exports,
                Error,
                Date,
                setTimeout(fn, delay) {
                    const id = ++timerId
                    timers.set(id, { fn, delay })
                    return id
                },
                clearTimeout(id) {
                    timers.delete(id)
                },
                require(name) {
                    if (name === 'vue')
                        return {
                            ...vue,
                            onActivated: (fn) => {
                                hooks.activate = fn
                            },
                            onDeactivated: (fn) => {
                                hooks.deactivate = fn
                            },
                            onUnmounted: (fn) => {
                                hooks.unmount = fn
                            },
                        }
                    if (name === 'vue-router')
                        return {
                            useRoute: () => route,
                            useRouter: () => router,
                        }
                    if (name.endsWith('/monitorApi')) return api
                    if (name.endsWith('/musicApi'))
                        return {
                            fetchCreatedPlaylists: async () => ({
                                playlists: [
                                    {
                                        id: '765432',
                                        dirid: '7',
                                        name: '个人歌单',
                                    },
                                ],
                            }),
                        }
                    if (name.endsWith('/settingsStore'))
                        return {
                            useSettingsStore: () => ({
                                settings: { defaultQuality: 'ask' },
                            }),
                        }
                    assert.ok(
                        name.startsWith('.'),
                        `Unexpected import: ${name}`,
                    )
                    return load(path.resolve(path.dirname(file), `${name}.ts`))
                },
            },
            { filename: file },
        )
        return exports
    }
    const scope = vue.effectScope()
    return {
        api,
        route,
        hooks,
        timers,
        scope,
        create: () =>
            scope.run(() =>
                load('src/composables/useMonitorPage.ts').useMonitorPage(),
            ),
        load,
    }
}

test('polling keeps its interval and stops while the cached page is inactive', async (t) => {
    let reads = 0
    const h = harness({
        getLibrary: async () => {
            reads++
            return {}
        },
    })
    t.after(() => h.scope.stop())
    h.create()
    h.hooks.activate()
    h.hooks.activate()
    await settle()
    assert.equal(reads, 1)
    assert.equal(h.timers.size, 1)
    assert.equal([...h.timers.values()][0].delay, 15000)
    h.hooks.deactivate()
    assert.equal(h.timers.size, 0)
    h.hooks.activate()
    await settle()
    assert.equal(reads, 2)
    h.hooks.unmount()
    assert.equal(h.timers.size, 0)
})

test('pending selections span pages, cap at 200 and reset on search or monitor changes', async (t) => {
    const h = harness()
    t.after(() => h.scope.stop())
    const p = h.create()
    p.songs.value = Array.from({ length: 30 }, (_, i) => entry(`song-${i}`))
    await vue.nextTick()
    p.selectPagePending(true)
    assert.equal(p.selectedMids.value.length, 30)
    p.page.value = 2
    p.songs.value = Array.from({ length: 30 }, (_, i) =>
        entry(`song-${i + 30}`),
    )
    await vue.nextTick()
    p.selectPagePending(true)
    assert.equal(p.selectedMids.value.length, 60)
    for (let i = 60; i < 220; i++) p.selectSong(`song-${i}`, true)
    assert.equal(p.selectedMids.value.length, 200)
    p.songs.value = [entry('song-0'), entry('song-1', 'ignored')]
    await vue.nextTick()
    assert.equal(p.selectedMids.value.length, 199)
    assert.ok(p.selectedMids.value.includes('song-0'))
    assert.ok(!p.selectedMids.value.includes('song-1'))
    assert.ok(p.selectedMids.value.includes('song-60'))
    assert.equal(p.page.value, 2)
    p.query.value = '虚构歌手'
    await vue.nextTick()
    assert.equal(p.selectedMids.value.length, 0)
    assert.equal(p.pageSongs.value.length, 2)
    p.selectSong('song-0', true)
    p.selectedId.value = 'monitor-b'
    await vue.nextTick()
    assert.equal(p.selectedMids.value.length, 0)
})

test('playlist prefill, fixed-quality validation and editing keep their existing contracts', async (t) => {
    const calls = []
    const h = harness({
        saveMonitor: async (...args) => {
            calls.push(plain(args))
            return monitor()
        },
    })
    t.after(() => {
        h.hooks.unmount()
        h.scope.stop()
    })
    h.route.query = {
        add: '1',
        source: 'created',
        playlistId: '765432',
        dirid: '7',
        name: '个人歌单',
    }
    const p = h.create()
    h.hooks.activate()
    await settle()
    assert.equal(p.showEditor.value, true)
    assert.equal(p.form.value.quality, '')
    assert.equal(p.form.value.dirid, '7')
    assert.deepEqual(plain(h.route.query), {})
    await p.save()
    assert.match(p.editorError.value, /请选择自动下载音质/)
    assert.equal(calls.length, 0)
    p.form.value.quality = 'flac'
    await p.save()
    assert.equal(calls.length, 1)
    assert.equal(calls[0][0].playlistId, '765432')
    assert.equal(p.showEditor.value, false)
    p.edit(monitor())
    assert.equal(p.editingId.value, 'monitor-a')
    await p.toggle(monitor(), true)
    assert.equal(calls[1][1], 'monitor-a')
    assert.equal(calls[1][0].enabled, true)
    await p.loadPlaylists()
    p.choosePlaylist(0)
    assert.equal(p.form.value.name, '个人歌单')
})

test('batch decisions, file linking and deletion still use the selected monitor and song', async (t) => {
    const calls = []
    let rows = [monitor()]
    const h = harness({
        getMonitors: async () => ({
            monitors: rows,
            running: false,
            persistenceFailed: false,
        }),
        decideMonitorSongs: async (...args) => {
            calls.push(plain(args))
            return { count: 1 }
        },
        decideSong: async (...args) => {
            calls.push(plain(args))
        },
        deleteMonitor: async (id) => {
            calls.push(id)
            rows = []
        },
    })
    t.after(() => h.scope.stop())
    const p = h.create()
    await p.selectMonitor('monitor-a')
    p.selectSong('song-a', true)
    await p.batchDecide('ignore')
    assert.deepEqual(calls[0], ['monitor-a', ['song-a'], 'ignore'])
    assert.equal(p.selectedMids.value.length, 0)
    assert.equal(p.batchMessage.value, '已忽略 1 首歌曲')
    p.openLink(entry('song-b'))
    p.linkPath.value = '/music/synthetic.wav'
    await p.confirmLink()
    assert.deepEqual(calls[1], [
        'monitor-a',
        'song-b',
        'link',
        '/music/synthetic.wav',
    ])
    await p.decide(entry('song-b'), 'reset')
    assert.deepEqual(calls[2], ['monitor-a', 'song-b', 'reset'])
    assert.equal(p.showLink.value, false)
    await p.remove(monitor())
    assert.equal(p.selectedId.value, '')
    assert.equal(p.songs.value.length, 0)
    assert.equal(p.busy.value, false)
})

test('failed batch and link actions retain user choices for retry', async (t) => {
    const h = harness({
        decideMonitorSongs: async () => {
            throw new Error('状态已变化')
        },
        decideSong: async () => {
            throw new Error('文件不可访问')
        },
    })
    t.after(() => h.scope.stop())
    const p = h.create()
    await p.selectMonitor('monitor-a')
    p.selectSong('song-a', true)
    await p.batchDecide('download')
    assert.deepEqual(plain(p.selectedMids.value), ['song-a'])
    assert.equal(p.error.value, '状态已变化')
    p.openLink(entry('song-a'))
    p.linkPath.value = '/music/missing.wav'
    await p.confirmLink()
    assert.equal(p.showLink.value, true)
    assert.equal(p.linkPath.value, '/music/missing.wav')
    assert.equal(p.linkError.value, '文件不可访问')
})

test('history for an older monitor cannot replace the newly opened history', async (t) => {
    const requests = new Map()
    const h = harness({
        getMonitorHistory: (id) =>
            new Promise((resolve) => requests.set(id, resolve)),
    })
    t.after(() => h.scope.stop())
    const p = h.create()
    const first = p.openHistory(monitor('first'))
    const second = p.openHistory(monitor('second'))
    requests.get('second')([{ startedAt: 2 }])
    await second
    requests.get('first')([{ startedAt: 1 }])
    await first
    assert.equal(p.historyName.value, 'second')
    assert.deepEqual(plain(p.history.value), [{ startedAt: 2 }])
    assert.equal(p.historyPage.value, 1)
    assert.equal(p.historyLoading.value, false)
})

test('server pages discard late responses and send filtering parameters', async (t) => {
    const requests = []
    const h = harness({
        getMonitorSongs: (...args) =>
            new Promise((resolve) => requests.push({ args, resolve })),
    })
    t.after(() => h.scope.stop())
    const p = h.create()
    const first = p.selectMonitor('monitor-a')
    await settle()
    assert.deepEqual(plain(requests[0].args), ['monitor-a', 1, 'all', ''])
    p.page.value = 2
    await vue.nextTick()
    assert.deepEqual(plain(requests[1].args), ['monitor-a', 2, 'all', ''])
    requests[1].resolve({
        items: [entry('page-2')],
        total: 60,
        page: 2,
        pageSize: 30,
    })
    await settle()
    requests[0].resolve({
        items: [entry('stale-page-1')],
        total: 60,
        page: 1,
        pageSize: 30,
    })
    await first
    assert.equal(p.page.value, 2)
    assert.equal(p.totalSongs.value, 60)
    assert.equal(p.songs.value[0].song.mid, 'page-2')
    p.filter.value = 'failed'
    await vue.nextTick()
    assert.deepEqual(plain(requests.at(-1).args), [
        'monitor-a',
        1,
        'failed',
        '',
    ])
    requests.at(-1).resolve({ items: [], total: 0, page: 1, pageSize: 30 })
    await settle()
    assert.equal(p.totalSongs.value, 0)
})

test('candidate refresh and explicit retry carry the selected monitor context', async (t) => {
    const calls = []
    const h = harness({
        decideSong: async (...args) => calls.push(plain(args)),
    })
    t.after(() => h.scope.stop())
    const p = h.create()
    await p.selectMonitor('monitor-a')
    await p.decide(entry('song-a'), 'refresh')
    await p.decide(entry('song-a', 'network_failed'), 'retry')
    assert.deepEqual(calls, [
        ['monitor-a', 'song-a', 'refresh'],
        ['monitor-a', 'song-a', 'retry'],
    ])
})
