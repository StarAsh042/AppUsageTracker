// 两套测试环境：纯逻辑跑 Node，组件跑 jsdom。
// 拆成 projects 是因为组件测试要加载 Svelte 插件和 jsdom，纯逻辑测试用不到，
// 混在一起只会让纯逻辑那部分也背上 jsdom 的启动开销。
import { defineConfig } from 'vitest/config';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  test: {
    projects: [
      {
        test: {
          name: 'logic',
          environment: 'node',
          include: ['src/lib/**/*.test.ts'],
        },
      },
      {
        plugins: [svelte()],
        // 不加 browser 条件时 Svelte 会解析到服务端构建，
        // 组件一 mount 就报 lifecycle_function_unavailable。
        resolve: { conditions: ['browser'] },
        test: {
          name: 'components',
          environment: 'jsdom',
          include: ['src/**/*.svelte.test.ts'],
        },
      },
    ],
  },
});
