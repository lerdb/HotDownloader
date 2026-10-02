// Run after cargo build; only synthetic local data, no music service requests.
import assert from 'node:assert/strict'
import { spawn } from 'node:child_process'
import { once } from 'node:events'
import fs from 'node:fs'
import net from 'node:net'
import os from 'node:os'
import path from 'node:path'
import { DatabaseSync } from 'node:sqlite'
import { setTimeout as delay } from 'node:timers/promises'

const workspace = path.resolve(import.meta.dirname, '..')
const fixture = fs.mkdtempSync(
    path.join(os.tmpdir(), 'hotdownloader-monitor-http-'),
)
const downloads = path.join(fixture, 'downloads')
fs.mkdirSync(downloads)
const filePath = path.join(downloads, 'Synthetic song - Synthetic artist.wav')
const wav = Buffer.alloc(52)
wav.write('RIFF')
wav.writeUInt32LE(44, 4)
wav.write('WAVEfmt ', 8)
wav.writeUInt32LE(16, 16)
wav.writeUInt16LE(1, 20)
wav.writeUInt16LE(1, 22)
wav.writeUInt32LE(8000, 24)
wav.writeUInt32LE(16000, 28)
wav.writeUInt16LE(2, 32)
wav.writeUInt16LE(16, 34)
wav.write('data', 36)
wav.writeUInt32LE(8, 40)
fs.writeFileSync(filePath, wav)
const db = new DatabaseSync(path.join(fixture, 'library.sqlite3'))
db.exec(
    'CREATE TABLE records(kind TEXT NOT NULL, key TEXT NOT NULL, data TEXT NOT NULL, PRIMARY KEY(kind,key)); PRAGMA user_version=1;',
)
const insert = db.prepare('INSERT INTO records VALUES(?,?,?)')
const mids = [
    'fictionalConfirm',
    'fictionalMatched',
    'fictionalFailed',
    'fictionalDone',
    'fictionalIgnored',
]
const states = [
    'pending_confirmation',
    'matched',
    'download_failed',
    'downloaded',
    'ignored',
]
const names = [
    '测试歌曲：候选待确认',
    '测试歌曲：已关联',
    '测试歌曲：失败',
    '测试歌曲：已下载',
    '测试歌曲：已忽略',
]
for (let i = 0; i < mids.length; i++) {
    insert.run(
        'entry',
        mids[i],
        JSON.stringify({
            song: {
                platform: 'qqmusic',
                id: i + 1,
                mid: mids[i],
                title: names[i],
                artist: '虚构歌手甲、虚构歌手乙',
                album: '合成测试专辑',
                qualities: [],
            },
            identity: {
                title: names[i],
                artists: ['虚构歌手甲', '虚构歌手乙'],
            },
            state: states[i],
            quality: '320kmp3',
            taskId: null,
            owned: true,
            path: i === 1 ? filePath : null,
            candidates: [],
            message:
                i === 0
                    ? '多个文件可能匹配，请选择处理方式'
                    : '仅用于本地验证的虚构数据',
            retries: i === 2 ? 3 : 0,
            nextRetry: 0,
            forceDownload: false,
        }),
    )
}
const config = {
    name: '虚构歌单 · 本地验证',
    source: 'public',
    playlistId: '123456',
    dirid: '',
    quality: '320kmp3',
    intervalMinutes: 60,
    enabled: false,
}
insert.run(
    'monitor',
    'fixture',
    JSON.stringify({
        id: 'fixture',
        ...config,
        members: mids,
        lastCheck: 0,
        nextCheck: 0,
        lastResult: '使用虚构数据验证逐曲操作',
        requested: false,
        draining: false,
    }),
)
db.close()

const probe = net.createServer()
probe.listen(0, '127.0.0.1')
await once(probe, 'listening')
const port = probe.address().port
await new Promise((resolve) => probe.close(resolve))
const binary = path.resolve(
    process.env.CARGO_TARGET_DIR ||
        path.join(workspace, 'crates/hotdownloader-server/target'),
    'debug',
    `hotdownloader-server${process.platform === 'win32' ? '.exe' : ''}`,
)
const child = spawn(binary, [], {
    cwd: workspace,
    windowsHide: true,
    stdio: ['ignore', 'ignore', 'pipe'],
    env: {
        ...process.env,
        HOTDOWNLOADER_DATA_DIR: fixture,
        HOTDOWNLOADER_DOWNLOAD_DIR: downloads,
        HOTDOWNLOADER_SCAN_DIRS: '[]',
        HOTDOWNLOADER_BIND: `127.0.0.1:${port}`,
        HOTDOWNLOADER_WEB_DIR: path.join(workspace, 'dist'),
        AUTH_USERNAME: '',
        AUTH_PASSWORD: '',
        HOTDOWNLOADER_TOKEN: 'fixture-monitor-access',
    },
})
let stderr = ''
child.stderr.on('data', (chunk) => {
    stderr += chunk
})
const childExit = once(child, 'exit')
const base = `http://127.0.0.1:${port}`
async function request(url, method = 'GET', body) {
    return fetch(base + url, {
        method,
        headers: {
            Authorization: 'Bearer fixture-monitor-access',
            'Content-Type': 'application/json',
        },
        body: body === undefined ? undefined : JSON.stringify(body),
    })
}
try {
    let started = false
    for (let i = 0; i < 100; i++) {
        if (child.exitCode !== null) throw new Error(stderr)
        try {
            if ((await fetch(base + '/healthz')).ok) {
                started = true
                break
            }
        } catch {}
        await delay(50)
    }
    assert.ok(started, `Server did not start: ${stderr}`)
    assert.equal((await fetch(base + '/api/library')).status, 401)
    assert.equal((await request('/api/library/scan', 'POST')).status, 200)
    const library = await (await request('/api/library')).json()
    assert.equal(library.fileCount, 1)
    assert.equal(library.error, null)
    assert.equal(
        (await request('/api/monitors', 'POST', { ...config, quality: 'ask' }))
            .status,
        400,
    )
    const created = await (
        await request('/api/monitors', 'POST', {
            ...config,
            name: '虚构歌单乙',
        })
    ).json()
    assert.ok(created.id)
    assert.equal(
        (
            await request(`/api/monitors/${created.id}`, 'PATCH', {
                ...config,
                name: '修改后的虚构歌单',
                intervalMinutes: 15,
            })
        ).status,
        200,
    )
    assert.equal(
        (await (await request('/api/monitors/fixture/songs')).json()).length,
        5,
    )
    assert.equal(
        (
            await request('/api/library/songs/fictionalIgnored', 'POST', {
                action: 'ignore',
            })
        ).status,
        200,
    )
    assert.equal(
        (
            await request('/api/library/songs/fictionalMatched', 'POST', {
                action: 'link',
                path: 'outside-the-library.wav',
            })
        ).status,
        400,
    )
    const indexedPath = fs.realpathSync.native(filePath)
    // Rust canonicalizes Windows paths with an extended-length prefix.
    const rustPath =
        process.platform === 'win32' && !indexedPath.startsWith('\\\\?\\')
            ? `\\\\?\\${indexedPath}`
            : indexedPath
    assert.equal(
        (
            await request('/api/library/songs/fictionalMatched', 'POST', {
                action: 'link',
                path: rustPath,
            })
        ).status,
        200,
    )
    assert.deepEqual(fs.readFileSync(filePath), wav)
    assert.equal((await (await request('/api/tasks')).json()).length, 0)
    console.log('Monitor HTTP smoke checks passed (synthetic data only).')
    if (process.argv.includes('--serve')) {
        console.log(`Fixture preview: ${base}/#/playlist/monitors`)
        console.log('Fixture token: fixture-monitor-access')
        await new Promise((resolve) => {
            process.once('SIGINT', resolve)
            process.once('SIGTERM', resolve)
        })
    }
} finally {
    child.kill()
    await childExit
    const resolved = fs.realpathSync(fixture)
    assert.equal(path.dirname(resolved), fs.realpathSync(os.tmpdir()))
    assert.ok(path.basename(resolved).startsWith('hotdownloader-monitor-http-'))
    fs.rmSync(resolved, { recursive: true })
}
