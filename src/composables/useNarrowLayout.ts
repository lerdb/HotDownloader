import { onScopeDispose, readonly, ref } from 'vue'

export const NARROW_LAYOUT_QUERY = '(max-width: 767px)'

/** 布局只由可用宽度决定；Android 等原生能力仍由平台检测决定。 */
export function useNarrowLayout() {
    // 初始化时读取当前宽度，避免窄屏先显示一次桌面布局。
    const mediaQuery = typeof window === 'undefined'
        ? null
        : window.matchMedia(NARROW_LAYOUT_QUERY)
    const isNarrow = ref(mediaQuery?.matches ?? false)

    function update(event: MediaQueryListEvent) {
        isNarrow.value = event.matches
    }

    mediaQuery?.addEventListener('change', update)

    // 跟随组件作用域释放监听，避免反复挂载时累积事件处理函数。
    onScopeDispose(() => {
        mediaQuery?.removeEventListener('change', update)
    })

    return readonly(isNarrow)
}
