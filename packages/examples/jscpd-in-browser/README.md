Webpack 4 + TypeScript starter
------------------------------

[![Build Status](https://travis-ci.org/juristr/webpack-typescript-starter.svg?branch=master)](https://travis-ci.org/juristr/webpack-typescript-starter)

This is by far no sophisticated starter or whatever. The goal is to get the simplest possible setup to get you started with Webpack 4 and TypeScript. You can then start from here and add further stuff you need, such as SASS compilation, add framework specific stuff etc.

Also check out the official Webpack 4 docs for a proper TypeScript setup: https://webpack.js.org/guides/typescript/

_Note, this is still a WIP. Contributions/suggestions are welcome :smiley:_

## Features

- [x] Webpack 4
- [x] TypeScript 2 compilation
- [x] ts-lint
- [x] Webpack Development Server
- [x] Karma and Jasmine test execution

## How to use

Just clone it and get going.

```
# --depth 1 removes all but one .git commit history
$ git clone --depth=1 https://github.com/juristr/webpack-typescript-starter.git <your-project-name>

# change directory to your project
cd  <your-project-name>

# Maybe remove the `.git` directory and start with a fresh one.

# install all dependencies.
$ npm i

# Start developing and serve your app:
npm start

# Build your app without minification: 
npm run build

# Build your app with minification: 
npm run build.all

# run unit tests:
npm run test
```
## Contributions

Of course! Open an issue and let's discuss :smiley:.

## Links and other useful stuff

Similar projects that could be a point of inspiration:

- [krasimir/webpack-library-starter](https://github.com/krasimir/webpack-library-starter)

