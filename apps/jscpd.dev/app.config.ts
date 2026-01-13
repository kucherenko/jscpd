export default defineAppConfig({
  ui: {
    colors: {
      primary: 'sky',
      neutral: 'slate'
    },
    primary: '#007bff',
    secondary: '#B200B2'
  },
  seo: {
    siteName: 'jscpd - Copy/Paste Detector'
  },
  site: {
    url: 'https://jscpd.dev',
    name: 'jscpd',
    description: 'Copy/paste detector for programming source code',
    ogImage: '/favicon.svg'
  },
  toc: {
    title: 'On this page',
    bottom: {
      title: 'Community',
      edit: 'https://github.com/kucherenko/jscpd/edit/master/docs/content',
      links: [
        {
          icon: 'simple-icons-github',
          label: 'Star on GitHub',
          to: 'https://github.com/kucherenko/jscpd',
          target: '_blank'
        },
        {
          icon: 'simple-icons-npm',
          label: 'View on npm',
          to: 'https://www.npmjs.com/package/jscpd',
          target: '_blank'
        },
        {
          icon: 'simple-icons-opencollective',
          label: 'Sponsor',
          to: 'https://opencollective.com/jscpd',
          target: '_blank'
        }
      ]
    }
  },
  header: {
    title: 'jscpd',
    to: '/',
    logo: {
      light: '/logo.svg',
      dark: '/logo-dark.svg'
    }
  },
  footer: {
    credits: 'Copyright © 2013-2025 Andrey Kucherenko'
  }
})
