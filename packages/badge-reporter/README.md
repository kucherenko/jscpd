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

[MIT](LICENSE) © Andrey Kucherenko
