import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'
import path from 'path'
import tauriConf from './src-tauri/tauri.conf.json' with { type: 'json' }

// https://vite.dev/config/
export default defineConfig({
  plugins: [vue()],
  resolve: {
    // 设置路径别名，让 import 更简洁（比如 import '@/utils'）
    alias: {
      '@': path.resolve(import.meta.dirname, './src'),
      '@components': path.resolve(import.meta.dirname, './src/components'),
    },
    // 导入时省略的扩展名（默认已支持 .js, .ts, .jsx, .tsx, .json）
    extensions: ['.mjs', '.js', '.ts', '.jsx', '.tsx', '.json', '.vue'],
  },
  // 防止 Vite 清除 Rust 显示的错误
  clearScreen: false,
  server: {
    watch: {
      // 告诉 Vite 忽略监听 `src-tauri` 目录
      // 同时忽略编辑器/工具链“原子写”产生的临时文件与临时目录：
      // 这类文件写完即被重命名/删除，Windows 上 chokidar 监听它们会抛 EBUSY，
      // 未捕获时会导致 dev server（以及 tauri dev）直接退出。
      ignored: ['**/src-tauri/**', '**/.*.tmpdir/**', '**/*.tmp'],
    },
    // 热更新（HMR）配置
    hmr: {
      overlay: true, // 报错时是否在浏览器遮罩层显示
    },
  },
  // 默认只有 VITE_ 开头的变量会暴露给客户端
  // 添加有关当前构建目标的额外前缀，使这些 CLI 设置的 Tauri 环境变量可以在客户端代码中访问
  envPrefix: ['VITE_', 'TAURI_ENV_*'],
  build: {
    // 在 debug 构建中不使用 minify
    minify: !process.env.TAURI_ENV_DEBUG ? 'oxc' : false,
    // 在 debug 构建中生成 sourcemap
    sourcemap: !!process.env.TAURI_ENV_DEBUG,
  },
  define: {
    // 把版本号注入环境变量
    'import.meta.env.VITE_APP_VERSION': JSON.stringify(tauriConf.version)
  },
})
