export interface SubmissionProgress {
    running: boolean
    stopping: boolean
    processed: number
    total: number
}

/** 停止后不再发起下一项；已经提交的请求完成后再报告整批停止。 */
export function createBatchSubmission(
    update: (state: SubmissionProgress) => void,
    interval = 100,
) {
    let controller: AbortController | undefined
    let pending: Promise<void> | undefined
    let state: SubmissionProgress = {
        running: false,
        stopping: false,
        processed: 0,
        total: 0,
    }
    const publish = () => update({ ...state })
    return {
        run<T>(
            items: readonly T[],
            submit: (item: T, signal: AbortSignal) => Promise<unknown>,
        ): Promise<void> {
            if (state.running)
                return Promise.reject(
                    new Error('已有一批任务正在提交，请先停止或等待完成'),
                )
            const current = new AbortController()
            controller = current
            state = {
                running: true,
                stopping: false,
                processed: 0,
                total: items.length,
            }
            publish()
            pending = (async () => {
                try {
                    for (const item of items) {
                        if (current.signal.aborted) break
                        await submit(item, current.signal)
                        state.processed++
                        publish()
                        if (
                            !current.signal.aborted &&
                            state.processed < state.total
                        ) {
                            await new Promise<void>((resolve) => {
                                const done = () => {
                                    clearTimeout(timer)
                                    current.signal.removeEventListener(
                                        'abort',
                                        done,
                                    )
                                    resolve()
                                }
                                const timer = setTimeout(done, interval)
                                current.signal.addEventListener('abort', done, {
                                    once: true,
                                })
                            })
                        }
                    }
                } finally {
                    state.running = false
                    state.stopping = false
                    controller = undefined
                    publish()
                }
            })()
            return pending
        },
        stop(): Promise<void> {
            if (state.running) {
                state.stopping = true
                controller?.abort()
                publish()
            }
            return pending?.catch(() => {}) ?? Promise.resolve()
        },
    }
}
