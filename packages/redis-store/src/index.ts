import {IStore} from '@jscpd/core';
import {IMapFrame} from '@jscpd/tokenizer';

const Redis = require("ioredis");

export default class RedisStore implements IStore<IMapFrame> {
  private name: string;
  private redis

  constructor() {
    this.redis = new Redis();
  }

  close(): void {
    this.redis.disconnect();
  }

  get(key: string): Promise<IMapFrame> {
    return this.redis.get(this.name + ':' + key).then(value => {
      if (!value) {
        throw new Error('not found')
      }
      return JSON.parse(value)
    });
  }

  namespace(name: string): void {
    this.name = name;
  }

  set(key: string, value: IMapFrame): Promise<IMapFrame> {
    return this.redis.set(this.name + ':' + key, JSON.stringify(value));
  }
}
