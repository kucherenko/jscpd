import {IStore} from '@jscpd/core';
import {IMapFrame} from '@jscpd/tokenizer';

// TODO implement the class
export default class RedisStore implements IStore<IMapFrame> {
  private name: string;

  close(): void {
    console.log('!!!');
  }

  get(key: string): Promise<IMapFrame> {
    return Promise.resolve(undefined);
  }

  namespace(name: string): void {
    this.name = name;
  }

  set(key: string, value: IMapFrame): Promise<IMapFrame> {
    return Promise.resolve(undefined);
  }


}
