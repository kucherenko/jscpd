import {IStore} from '@jscpd/core';
import {IMapFrame} from '@jscpd/tokenizer';
import {ensureDirSync} from 'fs-extra';
import * as rimraf from 'rimraf';

const level = require('level');

export default class LevelDbStore implements IStore<IMapFrame> {
  private name: string;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  private dbs: Record<string, any> = {};

  get(key: string): Promise<IMapFrame> {
    return this.dbs[this.name].get(key).then((value: string) => JSON.parse(value));
  }

  namespace(name: string): void {
    this.name = name;
    if (!(name in this.dbs)) {
      const path = `.jscpd/${name}`;
      rimraf.sync(path);
      ensureDirSync(path);
      this.dbs[name] = level(path);
    }
  }

  set(key: string, value: IMapFrame): Promise<IMapFrame> {
    return this.dbs[this.name].put(key, JSON.stringify(value));
  }

  close(): void {
    Object.entries(this.dbs).forEach(([name, db]) => {
      db.close(() => {
        rimraf('.jscpd/' + name, {maxBusyTries: 10}, (err) => {
          if (err) {
            console.log(err);
          }
        });
      });
    });
    rimraf.sync('.jscpd');
  }
}
