import type { GlobalThemeOverrides } from 'naive-ui'

// 深浅主题共用组件的字号与圆角，避免主题切换时改变布局尺寸。
const common: NonNullable<GlobalThemeOverrides['common']> = {
    fontFamily: '-apple-system, BlinkMacSystemFont, "Segoe UI", "Microsoft YaHei", sans-serif',
    fontSize: '14px',
    borderRadius: '8px',
    borderRadiusSmall: '6px',
}

// 组件颜色与 style.css 中的浅色页面背景、边框和文字变量对应。
export const lightThemeOverrides: GlobalThemeOverrides = {
    common: {
        ...common,
        primaryColor: '#178366',
        primaryColorHover: '#209675',
        primaryColorPressed: '#11654f',
        primaryColorSuppl: '#178366',
        bodyColor: '#f4f7f6',
        cardColor: '#ffffff',
        modalColor: '#ffffff',
        borderColor: '#dfe7e3',
        textColor1: '#202d28',
        textColor2: '#394a43',
        textColor3: '#6b7b73',
    },
}

// 深色模式保持同一套组件尺寸，只调整配色。
export const darkThemeOverrides: GlobalThemeOverrides = {
    common: {
        ...common,
        primaryColor: '#6ee7bc',
        primaryColorHover: '#92edcf',
        primaryColorPressed: '#4ed3a5',
        primaryColorSuppl: '#178366',
        bodyColor: '#111815',
        cardColor: '#1a2420',
        modalColor: '#1a2420',
        borderColor: '#314139',
        textColor1: '#e5eee9',
        textColor2: '#c7d5cd',
        textColor3: '#96aaa0',
    },
}
