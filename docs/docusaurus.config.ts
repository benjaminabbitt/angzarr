import {themes as prismThemes} from 'prism-react-renderer';
import type {Config} from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';

const config: Config = {
  title: 'Angzarr',
  tagline: 'Polyglot Event Sourcing & CQRS Framework',
  favicon: 'img/favicon.ico',

  future: {
    v4: true,
  },

  // GitHub Pages deployment
  url: 'https://angzarr.github.io',
  baseUrl: '/angzarr/',
  organizationName: 'angzarr',
  projectName: 'angzarr',
  trailingSlash: false,

  onBrokenLinks: 'throw',
  onBrokenMarkdownLinks: 'warn',

  i18n: {
    defaultLocale: 'en',
    locales: ['en'],
  },

  presets: [
    [
      'classic',
      {
        docs: {
          sidebarPath: './sidebars.ts',
          editUrl: 'https://github.com/angzarr/angzarr/tree/main/docs/',
          routeBasePath: '/', // Docs at root, no /docs prefix
        },
        blog: false, // Disable blog
        theme: {
          customCss: './src/css/custom.css',
        },
      } satisfies Preset.Options,
    ],
  ],

  themeConfig: {
    colorMode: {
      defaultMode: 'dark',
      respectPrefersColorScheme: true,
    },
    navbar: {
      title: 'Angzarr',
      items: [
        {
          type: 'docSidebar',
          sidebarId: 'docsSidebar',
          position: 'left',
          label: 'Documentation',
        },
        {
          href: 'https://github.com/angzarr/angzarr',
          label: 'GitHub',
          position: 'right',
        },
      ],
    },
    footer: {
      style: 'dark',
      links: [
        {
          title: 'Documentation',
          items: [
            {
              label: 'Getting Started',
              to: '/getting-started',
            },
            {
              label: 'Architecture',
              to: '/architecture',
            },
            {
              label: 'Components',
              to: '/components/aggregate',
            },
          ],
        },
        {
          title: 'Examples',
          items: [
            {
              label: 'Why Poker',
              to: '/examples/why-poker',
            },
            {
              label: 'Aggregates',
              to: '/examples/aggregates',
            },
            {
              label: 'Sagas',
              to: '/examples/sagas',
            },
          ],
        },
        {
          title: 'More',
          items: [
            {
              label: 'GitHub',
              href: 'https://github.com/angzarr/angzarr',
            },
            {
              label: 'PITCH.md',
              href: 'https://github.com/angzarr/angzarr/blob/main/PITCH.md',
            },
          ],
        },
      ],
      copyright: `Copyright © ${new Date().getFullYear()} Angzarr. Built with Docusaurus.`,
    },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
      additionalLanguages: [
        'bash',
        'rust',
        'go',
        'java',
        'csharp',
        'cpp',
        'protobuf',
        'yaml',
        'toml',
        'gherkin',
      ],
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
