import { Grammar, languages, Token as PrismToken, tokenize as PrismTokenize } from 'prismjs';
import { IToken } from '../interfaces/token/token.interface';
import { FORMATS } from './formats/formats';

const loadLanguages = require('prismjs/components/');

const ignore = {
  ignore: [
    {
      pattern: /(jscpd:ignore-start)[\s\S]*?(?=jscpd:ignore-end)/,
      lookbehind: true,
      greedy: true
    },
    {
      pattern: /jscpd:ignore-start/,
      greedy: false
    },
    {
      pattern: /jscpd:ignore-end/,
      greedy: false
    }
  ]
} as Grammar;

const punctuation = {
  new_line: /\n/,
  empty: /\s+/
} as Grammar;

(languages.markup as any).script.inside = {
  ...ignore,
  ...(languages.markup as any).script.inside,
  ...punctuation
};
(languages.markup as any).style.inside = {
  ...ignore,
  ...(languages.markup as any).style.inside,
  ...punctuation
};

function getLanguagePrismName(lang: string): string {
  if (FORMATS.hasOwnProperty(lang) && FORMATS[lang].parent) {
    return FORMATS[lang].parent as string;
  }
  return lang;
}

export function initLanguages(langs: string[]): void {
  loadLanguages(langs.map(getLanguagePrismName));
  Object.keys(languages).forEach(lang => {
    languages[lang] =
      typeof languages[lang] === 'object' ? { ...ignore, ...languages[lang], ...punctuation } : languages[lang];
  });
}

export function tokenize(code: string, language: string): IToken[] {
  let length = 0;
  let line = 1;
  let column = 1;

  initLanguages([language]);

  let tokens: IToken[] = [];

  PrismTokenize(code, languages[getLanguagePrismName(language)]).forEach(
    t => (tokens = tokens.concat(createTokens(t, language)))
  );

  function sanitizeLangName(name: string): string {
    return name && name.replace ? name.replace('language-', '') : 'unknown';
  }

  function createTokenFromString(token: string, lang: string): IToken[] {
    return [
      {
        format: lang,
        type: 'default',
        value: token,
        length: token.length
      } as IToken
    ];
  }

  function createTokenFromFlatToken(token: PrismToken, lang: string): IToken[] {
    return [
      {
        format: lang,
        type: token.type,
        value: token.content,
        length: (token as any).length
      } as IToken
    ];
  }

  function createTokens(token: PrismToken | string, lang: string): IToken[] {
    if (token instanceof PrismToken && typeof token.content === 'string') {
      return createTokenFromFlatToken(token, lang);
    }

    if (token instanceof PrismToken && Array.isArray(token.content)) {
      let res: IToken[] = [];
      token.content.forEach(
        t => (res = res.concat(createTokens(t, token.alias ? sanitizeLangName(token.alias as string) : lang)))
      );
      return res;
    }

    return createTokenFromString(token as string, lang);
  }

  function calculateLocation(token: IToken): IToken {
    const result: IToken = token;
    const lines: string[] = result.value.split('\n');
    const newLines = lines.length - 1;
    const start = {
      line,
      column
    };
    column = newLines !== 0 ? lines[lines.length - 1].length + 1 : column + lines[lines.length - 1].length;
    const end = {
      line: line + newLines,
      column
    };
    result.loc = { start, end };
    result.range = [length, length + result.length];
    length += result.length;
    line += newLines;
    return result;
  }

  return tokens.map(calculateLocation).filter((t: IToken) => {
    return t.format !== 'important' && t.format !== 'property' && t.format !== 'url';
  });
}
