export const SITE_NAME = 'PAS-Agent';
export const SITE_DESCRIPTION = 'Portable work sessions across AI coding agents.';
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
      { label: 'Architecture', slug: 'architecture' },
      { label: 'Development', slug: 'development' },
    ],
  },
];

export const docPages: DocPage[] = docSections.flatMap((s) => s.items);
