import {IMapFrame, IStore, MemoryStore} from '@jscpd-ai/core';
import {red} from 'colors/safe';

export function getStore(storeName: string | undefined): IStore<IMapFrame> {
  if (storeName) {
    const packageName = '@jscpd/' + storeName + '-store';
    try {
      const store = require(packageName).default;
      return new store();
    } catch (e) {
      console.error(red('store name ' + storeName + ' not installed.'))
    }
  }
  return new MemoryStore<IMapFrame>();
}
