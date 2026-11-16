import { defineConfig } from 'vitest/config';

// 纯逻辑单测：不加载 Svelte 插件，node 环境即可，启动更快。
export default defineConfig({
  test: {
    environment: 'node',
    include: ['src/**/*.test.ts'],
  },
});
