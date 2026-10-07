// Generate short silent fixtures with an existing FFmpeg installation.
import { mkdirSync } from 'node:fs'
import path from 'node:path'
import { spawnSync } from 'node:child_process'

const output = path.resolve(
    import.meta.dirname,
    '../crates/hotdownloader-server/testdata/audio',
)
const ffmpeg = process.env.FFMPEG_PATH || 'ffmpeg'
const version = spawnSync(ffmpeg, ['-version'], {
    encoding: 'utf8',
    windowsHide: true,
})
if (version.error || version.status !== 0)
    throw new Error('需要已安装的 FFmpeg；可用 FFMPEG_PATH 指定路径。')
console.log(version.stdout.split(/\r?\n/)[0])
mkdirSync(output, { recursive: true })
for (const [extension, codec] of [
    ['mp3', 'libmp3lame'],
    ['flac', 'flac'],
]) {
    const result = spawnSync(
        ffmpeg,
        [
            '-hide_banner',
            '-loglevel',
            'error',
            '-y',
            '-f',
            'lavfi',
            '-i',
            'anullsrc=r=44100:cl=mono',
            '-t',
            '0.12',
            '-map_metadata',
            '-1',
            '-c:a',
            codec,
            ...(extension === 'mp3'
                ? ['-b:a', '128k', '-write_xing', '0', '-id3v2_version', '0']
                : []),
            path.join(output, `silence.${extension}`),
        ],
        { encoding: 'utf8', windowsHide: true },
    )
    if (result.error || result.status !== 0)
        throw new Error(result.error?.message || result.stderr)
    console.log(`Generated silence.${extension}`)
}
