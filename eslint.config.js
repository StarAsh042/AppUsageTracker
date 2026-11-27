import js from '@eslint/js';
import svelte from 'eslint-plugin-svelte';
import tseslint from 'typescript-eslint';
import globals from 'globals';
import svelteConfig from './svelte.config.js';

export default tseslint.config(
  { ignores: ['dist/', 'node_modules/', 'target/', 'src-tauri/gen/', '.dsh-ops/'] },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  ...svelte.configs.recommended,
  {
    languageOptions: {
      globals: { ...globals.browser, ...globals.node },
    },
    rules: {
      // 组件里的 Map / Set / Date 都是 $derived 或事件处理里的局部量，
      // 用完就丢，不是要参与响应的状态；换成 SvelteMap 只会更慢更难读。
      'svelte/prefer-svelte-reactivity': 'off',
    },
  },
  {
    files: ['extension/**/*.js'],
    languageOptions: {
      globals: { ...globals.webextensions },
    },
  },
  {
    files: ['**/*.svelte'],
    languageOptions: {
      parserOptions: {
        // svelte-eslint-parser 解析 <script> 时交给 typescript-eslint。
        parser: tseslint.parser,
        extraFileExtensions: ['.svelte'],
        svelteConfig,
      },
    },
  },
);
