import {readFileSync, writeFileSync} from 'fs-extra';
import {sync} from 'fast-glob';

const files = sync('node_modules/prismjs/components/prism-*.min.js', {
  onlyFiles: true,
}).map((file: string) => readFileSync(file));

writeFileSync(process.argv[2], files.join('\n'));

