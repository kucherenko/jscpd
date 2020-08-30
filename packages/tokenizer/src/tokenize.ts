import * as reprism from 'reprism';
import {FORMATS} from './formats';
import {createTokensMaps, TokensMap} from './token-map';
import {IOptions, IToken} from '@jscpd/core';
import {loadLanguages} from './grammar-loader';

const ignore = {
  ignore: [
    {
      pattern: /(jscpd:ignore-start)[\s\S]*?(?=jscpd:ignore-end)/,
      lookbehind: true,
      greedy: true,
    },
    {
      pattern: /jscpd:ignore-start/,
      greedy: false,
    },
    {
      pattern: /jscpd:ignore-end/,
      greedy: false,
    },
  ],
};

const punctuation = {
  // eslint-disable-next-line @typescript-eslint/camelcase
  new_line: /\n/,
  empty: /\s+/,
};


const initializeFormats = (): void => {
  loadLanguages();
  Object
    .keys(reprism.default.languages)
    .forEach((lang: string) => {
      if (lang !== 'extend' && lang !== 'insertBefore' && lang !== 'DFS') {
        reprism.default.languages[lang] = {
          ...ignore,
          ...reprism.default.languages[lang],
          ...punctuation,
        }
      }
    });
}

initializeFormats();

function getLanguagePrismName(lang: string): string {
  if (lang in FORMATS && FORMATS[lang].parent) {
    return FORMATS[lang].parent;
  }
  return lang;
}

export function tokenize(code: string, language: string): IToken[] {
  let length = 0;
  let line = 1;
  let column = 1;

  function sanitizeLangName(name: string): string {
    return name && name.replace ? name.replace('language-', '') : 'unknown';
  }

  function createTokenFromString(token: string, lang: string): IToken[] {
    return [
      {
        format: lang,
        type: 'default',
        value: token,
        length: token.length,
      } as IToken,
    ];
  }

  function calculateLocation(token: IToken, position: number): IToken {
    const result: IToken = token;
    const lines: string[] = result.value.split('\n');
    const newLines = lines.length - 1;
    const start = {
      line,
      column,
      position
    };
    column = newLines !== 0 ? lines[lines.length - 1].length + 1 : column + lines[lines.length - 1].length;
    const end = {
      line: line + newLines,
      column,
      position
    };
    result.loc = {start, end};
    result.range = [length, length + result.length];
    length += result.length;
    line += newLines;
    return result;
  }


  function createTokenFromFlatToken(token: any, lang: string): IToken[] {
    return [
      {
        format: lang,
        type: token.type,
        value: token.content,
        length: token.length,
      } as IToken,
    ];
  }

  function createTokens(token: reprism.default.Token | string, lang: string): IToken[] {
    if (token.content && typeof token.content === 'string') {
      return createTokenFromFlatToken(token, lang);
    }

    if (token.content && Array.isArray(token.content)) {
      let res: IToken[] = [];
      token.content.forEach(
        (t) => (res = res.concat(createTokens(t, token.alias ? sanitizeLangName(token.alias as string) : lang))),
      );
      return res;
    }

    return createTokenFromString(token as string, lang);
  }


  let tokens: IToken[] = [];
  const grammar = reprism.default.languages[getLanguagePrismName(language)];
  reprism.default.tokenize(code, grammar)
    .forEach(
      (t) => (tokens = tokens.concat(createTokens(t, language))),
    );
  return tokens
    .filter((t: IToken) => t.format in FORMATS)
    .map(
      (token, index) => calculateLocation(token, index)
    );
}

export function createTokenMapBasedOnCode(id: string, data: string, format: string, options: Partial<IOptions> = {}): TokensMap[] {

  const {mode, ignoreCase} = options;

  const tokens: IToken[] = tokenize(data, format)
    .filter((token) => mode(token, options))

  if (ignoreCase) {
    return createTokensMaps(id, data, tokens.map(
      (token: IToken): IToken => {
        token.value = token.value.toLocaleLowerCase();
        return token;
      },
    ), options);
  }
  return createTokensMaps(id, data, tokens, options);
}
