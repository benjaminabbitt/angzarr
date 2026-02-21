import {themes as prismThemes} from 'prism-react-renderer';
import type {Config} from '@docusaurus/types';
import type * as Preset from '@docusaurus/preset-classic';
import codeSnippets from 'remark-code-snippets';

const config: Config = {
  title: '⍼ Angzarr',
  tagline: 'Polyglot Event Sourcing & CQRS Framework',
  favicon: 'img/favicon.ico',

  future: {
    v4: true,
  },

  markdown: {
    mermaid: true,
  },
  themes: ['@docusaurus/theme-mermaid'],

  // GitHub Pages deployment
  url: 'https://angzarr.io',
  baseUrl: '/',
  organizationName: 'benjaminabbitt',
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
          editUrl: 'https://github.com/benjaminabbitt/angzarr/tree/main/docs/',
          routeBasePath: '/', // Docs at root, no /docs prefix
          remarkPlugins: [
            [codeSnippets, {
              baseDir: '..',  // Repository root (one level up from docs/)
            }],
          ],
        },
        blog: false, // Disable blog
        theme: {
          customCss: './src/css/custom.css',
        },
      } satisfies Preset.Options,
    ],
  ],

  plugins: [
    // SDK documentation - each client README becomes a doc root
    [
      '@docusaurus/plugin-content-docs',
      {
        id: 'sdk-rust',
        path: '../client/rust',
        routeBasePath: 'sdk/rust',
        sidebarPath: false,
        remarkPlugins: [
          [codeSnippets, { baseDir: '..' }],
        ],
      },
    ],
    [
      '@docusaurus/plugin-content-docs',
      {
        id: 'sdk-go',
        path: '../client/go',
        routeBasePath: 'sdk/go',
        sidebarPath: false,
        remarkPlugins: [
          [codeSnippets, { baseDir: '..' }],
        ],
      },
    ],
    [
      '@docusaurus/plugin-content-docs',
      {
        id: 'sdk-python',
        path: '../client/python',
        routeBasePath: 'sdk/python',
        sidebarPath: false,
        remarkPlugins: [
          [codeSnippets, { baseDir: '..' }],
        ],
      },
    ],
    [
      '@docusaurus/plugin-content-docs',
      {
        id: 'sdk-java',
        path: '../client/java',
        routeBasePath: 'sdk/java',
        sidebarPath: false,
        remarkPlugins: [
          [codeSnippets, { baseDir: '..' }],
        ],
      },
    ],
    [
      '@docusaurus/plugin-content-docs',
      {
        id: 'sdk-csharp',
        path: '../client/csharp',
        routeBasePath: 'sdk/csharp',
        sidebarPath: false,
        remarkPlugins: [
          [codeSnippets, { baseDir: '..' }],
        ],
      },
    ],
    [
      '@docusaurus/plugin-content-docs',
      {
        id: 'sdk-cpp',
        path: '../client/cpp',
        routeBasePath: 'sdk/cpp',
        sidebarPath: false,
        remarkPlugins: [
          [codeSnippets, { baseDir: '..' }],
        ],
      },
    ],
    // Internal component documentation
    [
      '@docusaurus/plugin-content-docs',
      {
        id: 'internals-bus',
        path: '../src/bus',
        routeBasePath: 'internals/bus',
        sidebarPath: false,
        remarkPlugins: [
          [codeSnippets, { baseDir: '..' }],
        ],
      },
    ],
    [
      '@docusaurus/plugin-content-docs',
      {
        id: 'internals-storage',
        path: '../src/storage',
        routeBasePath: 'internals/storage',
        sidebarPath: false,
        remarkPlugins: [
          [codeSnippets, { baseDir: '..' }],
        ],
      },
    ],
  ],

  themeConfig: {
    colorMode: {
      defaultMode: 'dark',
      respectPrefersColorScheme: true,
    },
    navbar: {
      title: '⍼ Angzarr',
      items: [
        {
          type: 'docSidebar',
          sidebarId: 'docsSidebar',
          position: 'left',
          label: 'Documentation',
        },
        {
          href: 'https://github.com/benjaminabbitt/angzarr',
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
              href: 'https://github.com/benjaminabbitt/angzarr',
            },
            {
              label: 'PITCH.md',
              href: 'https://github.com/benjaminabbitt/angzarr/blob/main/PITCH.md',
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
