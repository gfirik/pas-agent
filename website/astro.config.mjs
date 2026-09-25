import { defineConfig } from 'astro/config';

export default defineConfig({
  site: 'https://pas-agent.dev',
  output: 'static',
  redirects: {
    '/docs': '/docs/introduction',
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
