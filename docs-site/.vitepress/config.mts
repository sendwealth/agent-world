import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'Agent World',
  description: 'A survival sandbox world for AI agents',
  lang: 'en',

  ignoreDeadLinks: [
    /^https?:\/\/localhost/,
  ],

  head: [
    ['link', { rel: 'icon', type: 'image/svg+xml', href: '/logo.svg' }],
  ],

  themeConfig: {
    logo: '/logo.svg',

    nav: [
      { text: 'Home', link: '/' },
      {
        text: 'Tutorial',
        items: [
          { text: 'Quick Start', link: '/getting-started/quick-start' },
          { text: 'Your First Agent', link: '/getting-started/your-first-agent' },
          { text: 'World Basics', link: '/getting-started/world-basics' },
        ],
      },
      {
        text: 'How-to',
        items: [
          { text: 'Deploy World Engine', link: '/how-to/deploy-world' },
          { text: 'Configure Agent', link: '/how-to/configure-agent' },
          { text: 'A2A Protocol', link: '/how-to/a2a-protocol' },
          { text: 'Custom Skills', link: '/how-to/custom-skills' },
          { text: 'Monitor Agents', link: '/how-to/monitor-agents' },
          { text: 'Third-Party Agent API', link: '/how-to/third-party-agent-api' },
          { text: 'Cross-World Interaction', link: '/how-to/cross-world-interaction' },
        ],
      },
      {
        text: 'Reference',
        items: [
          { text: 'API Reference', link: '/reference/api' },
          { text: 'CLI Reference', link: '/reference/cli' },
          { text: 'Config Schema', link: '/reference/config-schema' },
          { text: 'A2A Message Types', link: '/reference/a2a-message-types' },
          { text: 'Lifecycle Phases', link: '/reference/lifecycle-phases' },
        ],
      },
      {
        text: 'Explain',
        items: [
          { text: 'Architecture', link: '/explanation/architecture' },
          { text: 'Design Decisions', link: '/explanation/design-decisions' },
          { text: 'Why Token Economy', link: '/explanation/why-token-economy' },
          { text: 'Emergence Philosophy', link: '/explanation/emergence-philosophy' },
        ],
      },
      { text: 'ADR', link: '/adr/' },
      {
        text: '中文',
        link: '/zh/',
      },
    ],

    sidebar: {
      '/getting-started/': [
        {
          text: 'Tutorial',
          items: [
            { text: 'Quick Start', link: '/getting-started/quick-start' },
            { text: 'Your First Agent', link: '/getting-started/your-first-agent' },
            { text: 'World Basics', link: '/getting-started/world-basics' },
          ],
        },
      ],
      '/how-to/': [
        {
          text: 'How-to Guides',
          items: [
            { text: 'Deploy World Engine', link: '/how-to/deploy-world' },
            { text: 'Configure an Agent', link: '/how-to/configure-agent' },
            { text: 'Use A2A Protocol', link: '/how-to/a2a-protocol' },
            { text: 'Develop Custom Skills', link: '/how-to/custom-skills' },
            { text: 'Monitor Agents', link: '/how-to/monitor-agents' },
            { text: 'Third-Party Agent API', link: '/how-to/third-party-agent-api' },
            { text: 'Cross-World Interaction', link: '/how-to/cross-world-interaction' },
          ],
        },
      ],
      '/reference/': [
        {
          text: 'Reference',
          items: [
            { text: 'API Reference', link: '/reference/api' },
            { text: 'CLI Reference', link: '/reference/cli' },
            { text: 'Config Schema', link: '/reference/config-schema' },
            { text: 'A2A Message Types', link: '/reference/a2a-message-types' },
            { text: 'Lifecycle Phases', link: '/reference/lifecycle-phases' },
          ],
        },
      ],
      '/explanation/': [
        {
          text: 'Explanation',
          items: [
            { text: 'Architecture', link: '/explanation/architecture' },
            { text: 'Design Decisions', link: '/explanation/design-decisions' },
            { text: 'Why Token Economy', link: '/explanation/why-token-economy' },
            { text: 'Emergence Philosophy', link: '/explanation/emergence-philosophy' },
          ],
        },
      ],
      '/adr/': [
        {
          text: 'Architecture Decision Records',
          items: [
            { text: 'ADR Index', link: '/adr/' },
          ],
        },
      ],
      '/meta/': [
        {
          text: 'Meta',
          items: [
            { text: 'Contributing to Docs', link: '/meta/contributing-docs' },
            { text: 'Style Guide', link: '/meta/style-guide' },
            { text: 'Glossary', link: '/meta/glossary' },
          ],
        },
      ],
    },

    socialLinks: [
      { icon: 'github', link: 'https://github.com/sendwealth/agent-world' },
    ],

    search: {
      provider: 'local',
    },

    editLink: {
      pattern: 'https://github.com/sendwealth/agent-world/edit/main/docs-site/:path',
      text: 'Edit this page on GitHub',
    },

    footer: {
      message: 'Released under the <a href="https://github.com/sendwealth/agent-world/blob/main/LICENSE">MIT License</a>.',
      copyright: 'Copyright 2025-present Agent World Contributors',
    },
  },

  locales: {
    '/': {
      lang: 'en',
    },
    '/zh/': {
      lang: 'zh-CN',
      link: '/zh/',
    },
  },
})
