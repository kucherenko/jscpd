import {readJsonSync, writeJsonSync} from 'fs-extra';

function changePackageJsonFields(path: string, pairs: { name: string; value: string }[]): void {
  const pkg = readJsonSync(path);
  pairs.forEach((pair) => {
    pkg[pair.name] = pair.value;
  });
  writeJsonSync(path, pkg, {spaces: 2});
}

if (require.main === module) {
  const [, , path, value] = process.argv;
  changePackageJsonFields(path, [
    {name: 'main', value},
    {name: 'types', value},
  ]);
}
