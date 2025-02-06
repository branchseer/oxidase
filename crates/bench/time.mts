// https://github.com/oxc-project/bench-javascript-transformer-written-in-rust/blob/e298c6c3be57a4a48176595b5c558dabd98d0288/table.mjs


import { type Data, formatAsTable, readdir } from './utils.mjs'
import { $ } from 'zx'
import { rmSync } from 'node:fs'

const criterionDir = "../../target/criterion";

await $`cargo bench --bench time --no-run`;

if (process.argv.includes('--no-run')) {
  process.exit(0);
}

rmSync(criterionDir, { recursive: true, force: true });
await $`cargo bench --bench time`;

function moveToFront<T>(arr: T[], predicate: (val: T) => boolean): T[] {
  return [...arr.filter(predicate), ...arr.filter(val => !predicate(val))]
}


async function readData() {
  const data: Data = {};

  const groups = await readdir(criterionDir);
  for (const group of groups) {
    data[group] ||= {};

    const benches = moveToFront(await readdir(`${criterionDir}/${group}`), name => name === 'oxidase');
    for (const bench of benches) {
      data[group][bench] ||= {};

      const parameters = moveToFront(await readdir(`${criterionDir}/${group}/${bench}`), name => name.startsWith('original'));
      for (const parameter of parameters) {
        const json = await import(`${criterionDir}/${group}/${bench}/${parameter}/new/estimates.json`, { with: { type: "json" } });
        const durationMs = json.default.mean.point_estimate / 1_000_000;
        data[group][bench][parameter] ||= durationMs;
      }
    }
  }
  return data
}


const data = await readData();
console.log(formatAsTable(data, (durationMs) => `${durationMs.toFixed(2)} ms`))
