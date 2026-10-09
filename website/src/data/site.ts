import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

export const SITE_NAME = 'PAS-Agent';
export const SITE_DESCRIPTION = 'PAS-Agent (Portable Agent Sessions) carries your task, progress and decisions from one AI coding agent to the next: Claude Code, Codex, Cursor, Gemini CLI and more.';
export const GITHUB_URL = 'https://github.com/gfirik/pas-agent';

/**
 * The released version, read from the workspace `Cargo.toml` at build time so the site can
 * never drift from the code. `bun run build` runs from `website/`, one level below it.
 * Fails the build if the version cannot be found, rather than printing a wrong one.
 */
function readVersion(): string {
  const manifest = resolve(process.cwd(), '..', 'Cargo.toml');
  const text = readFileSync(manifest, 'utf8');
  const section = text.split(/^\[workspace\.package\]\s*$/m)[1]?.split(/^\[/m)[0] ?? '';
  const version = section.match(/^version\s*=\s*"([^"]+)"/m)?.[1];
  if (!version) throw new Error(`Could not read [workspace.package] version from ${manifest}`);
  return version;
}

export const VERSION = readVersion();
export const RELEASE_URL = `${GITHUB_URL}/releases/tag/v${VERSION}`;

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
