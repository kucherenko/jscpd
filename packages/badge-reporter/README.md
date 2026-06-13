# `@jscpd/badge-reporter`

> The badge reporter for [jscpd](https://github.com/kucherenko/jscpd).

Generate badges like that:

![jscpd-badge-green](https://raw.githubusercontent.com/kucherenko/jscpd/master/assets/jscpd-badge.svg?sanitize=true)

## Getting started

### Install

```bash
npm install @jscpd/badge-reporter
```

### Usage

```bash
jscpd [...options] --reporters badge /path/to/source
```

### Options

```typescript

interface IBadgeOptions {
  color?: string, // color of badge, if threshold > current - green, if  threshold < current - red, no threshold provided - grey
  subject?: string, // label of the badge, default "Copy/Paste"
  style?: string, // "flat" of undefined, default - undefined
  icon?: string, // 'data:image/svg+xml;base64,...' icon
  iconWidth?: number, // width of the icon
  path?: string, // path to badge, default is 'jscpd-badge.svg' in output folder
}

```

![ga tracker](https://www.google-analytics.com/collect?v=1&a=257770996&t=pageview&dl=https%3A%2F%2Fgithub.com%2Fkucherenko%2Fjscpd&ul=en-us&de=UTF-8&cid=978224512.1377738459&tid=UA-730549-17&z=887657232 "ga tracker")

[MIT](LICENSE) © Andrey Kucherenko
