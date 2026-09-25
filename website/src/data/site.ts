export const SITE_NAME = 'PAS-Agent';
export const SITE_DESCRIPTION = 'PAS-Agent (Portable Agent Sessions) carries your task, progress and decisions from one AI coding agent to the next: Claude Code, Codex, Cursor, Gemini CLI and more.';
export const GITHUB_URL = 'https://github.com/gfirik/pas-agent';

export interface DocPage {
  label: string;
  slug: string;
}

export const docSections: { title: string; items: DocPage[] }[] = [
  {
    title: 'Getting Started',
    items: [
      { label: 'Introduction', slug: 'introduction' },
      { label: 'Installation', slug: 'installation' },
      { label: 'Quick Start', slug: 'quick-start' },
      { label: 'Workflows', slug: 'workflows' },
    ],
  },
  {
    title: 'Concepts',
    items: [
      { label: 'Sessions', slug: 'sessions' },
      { label: 'Checkpoints', slug: 'checkpoints' },
      { label: 'Portable State', slug: 'portable-state' },
    ],
  },
  {
    title: 'Reference',
    items: [
      { label: 'CLI Reference', slug: 'cli-reference' },
      { label: 'Troubleshooting', slug: 'troubleshooting' },
      { label: 'Architecture', slug: 'architecture' },
      { label: 'Development', slug: 'development' },
    ],
  },
];

export const docPages: DocPage[] = docSections.flatMap((s) => s.items);

/** Prefixes a root-relative path with the deploy base (e.g. `/pas-agent` on GitHub Pages). */
export function withBase(path: string): string {
  return import.meta.env.BASE_URL.replace(/\/$/, '') + path;
}
