import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// 多页构建：主窗口 index.html + 悬浮窗 widget.html
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
  },
  build: {
    target: 'chrome105',
    rollupOptions: {
      input: {
        main: 'index.html',
        widget: 'widget.html',
      },
    },
  },
});
