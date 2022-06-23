import {IStore} from '@jscpd/core';
import {IMapFrame} from '@jscpd/tokenizer';
import {ensureDirSync} from 'fs-extra';
import * as rimraf from 'rimraf';
import {Level} from 'level';

export default class LevelDbStore implements IStore<IMapFrame> {
  private name: string;
  private dbs: Record<string, Level> = {};

  get(key: string): Promise<IMapFrame> {
    return this.dbs[this.name].get(key).then((value: string) => JSON.parse(value));
  }

  namespace(name: string): void {
    this.name = name;
    if (!(name in this.dbs)) {
      const path = `.jscpd/${name}`;
      rimraf.sync(path);
      ensureDirSync(path);
      this.dbs[name] = new Level(path);
    }
  }

  set(key: string, value: IMapFrame): Promise<IMapFrame> {
    return this.dbs[this.name]
      .put(key, JSON.stringify(value))
      .then(() => value);
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
