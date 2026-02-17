import { defineConfig } from 'astro/config';
import tailwind from '@astrojs/tailwind';
import sitemap from '@astrojs/sitemap';
import mdx from '@astrojs/mdx';

const repo = process.env.GITHUB_REPOSITORY?.split('/')[1] ?? '';
const owner = (process.env.GITHUB_REPOSITORY_OWNER ?? 'NOVA-ALLRounder').toLowerCase();
const isUserSiteRepo = repo.endsWith('.github.io');
const defaultBase = process.env.GITHUB_ACTIONS === 'true'
  ? (isUserSiteRepo ? '/' : `/${repo}/`)
  : '/';
const base = (process.env.PUBLIC_BASE_PATH ?? defaultBase).replace(/\/?$/, '/');

export default defineConfig({
  site: process.env.SITE_URL ?? `https://${owner}.github.io`,
  base,
  integrations: [
    tailwind(),
    sitemap(),
    mdx(),
  ],
  output: 'static',
});
