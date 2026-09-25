import type { APIRoute } from 'astro';
import { docPages, withBase } from '../data/site';

const paths = ['/', ...docPages.map((page) => `/docs/${page.slug}/`)];

export const GET: APIRoute = ({ site }) => {
  const urls = paths
    .map((path) => `  <url><loc>${new URL(withBase(path), site).href}</loc></url>`)
    .join('\n');
  const body = `<?xml version="1.0" encoding="UTF-8"?>
<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">
${urls}
</urlset>
`;
  return new Response(body, { headers: { 'Content-Type': 'application/xml' } });
};
