![jscpd logo](../assets/logo-small-box.svg)
# Programming API

## JSCPD  

Detect duplications in code string:

```typescript
import {
  JSCPD, 
  IClone,
  IOptions, 
} from 'jscpd';

const options: IOptions = {};

const cpd = new JSCPD(options);

const code = '...string with my code...';

cpd.detect(code, { id: 'test', format: 'markup' })
  .then((clones: IClone[]) => console.log(clones));
``` 
Detect duplications in files:

```typescript
import {
  JSCPD, 
  IClone,
  IOptions, 
} from 'jscpd';

const options: IOptions = {};

const cpd = new JSCPD(options);

cpd.detectInFiles(['./src', './tests'])
  .then((clones: IClone[]) => console.log(clones));
``` 

## Options

```typescript
export interface IOptions {
  executionId?: string;
  minLines?: number;
  maxLines?: number;
  maxSize?: string;
  minTokens?: number;
  threshold?: number;
  xslHref?: string;
  formatsExts?: { [key: string]: string[] };
  output?: string;
  path?: string[];
  mode?: string | ((token: IToken) => boolean);
  storeOptions?: IStoreManagerOptions;
  config?: string;
  ignore?: string[];
  format?: string[];
  reporters?: string[];
  listeners?: string[];
  blame?: boolean;
  cache?: boolean;
  silent?: boolean;
  debug?: boolean;
  list?: boolean;
  absolute?: boolean;
  gitignore?: boolean;
}
```

## Events

During the detections process `JSCPD` emit following events:

### MATCH_SOURCE_EVENT

New source detection started event

```typescript
cpd.on(MATCH_SOURCE_EVENT, (source) => {
  console.log(source);
});
```

### CLONE_FOUND_EVENT

Clone found event
```typescript
cpd.on(CLONE_FOUND_EVENT, (clone: IClone) => {
  console.log(clone);
});
```

### SOURCE_SKIPPED_EVENT

Skipped source event (see max-size, min-lines and max-lines options)
```typescript
cpd.on(SOURCE_SKIPPED_EVENT, (stat) => {
  console.log(stat);
});
```

### END_EVENT

Detection process finished event
```typescript
cpd.on(END_EVENT, (clones: IClone[]) => {
  console.log(clones);
});
```

## Reporters 

TODO

## Modes

TODO
