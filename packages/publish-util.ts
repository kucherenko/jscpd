import {readJsonSync, writeJsonSync} from 'fs-extra';

const changePackageJsonFields = (path: string, pairs: { name: string; value: string }[] = []): void => {
  const pkg = readJsonSync(path);
  pairs.forEach((pair: { name: string; value: string }) => {
    pkg[pair.name] = pair.value;
  });
  writeJsonSync(path, pkg, {spaces: 2});
};

if (!module.parent) {
  const [, , path, value] = process.argv;
  changePackageJsonFields(path, [
    {name: 'main', value},
    {name: 'types', value},
  ]);
}
