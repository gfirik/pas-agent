import { defineConfig } from 'astro/config';

export default defineConfig({
  site: 'https://gfirik.github.io',
  base: '/pas-agent',
  output: 'static',
  redirects: {
    '/docs': '/pas-agent/docs/introduction',
  },
  markdown: {
    shikiConfig: {
      theme: 'github-dark',
    },
  },
  build: {
    format: 'directory',
  },
});
