// https://github.com/oxc-project/bench-javascript-transformer-written-in-rust/blob/e298c6c3be57a4a48176595b5c558dabd98d0288/table.mjs


import { type Data, formatAsTable, readdir } from './utils.mjs'

function moveToFront<T>(arr: T[], predicate: (val: T) => boolean): T[] {
  return [...arr.filter(predicate), ...arr.filter(val => !predicate(val))]
}

async function readData() {
  const data: Data = {};
  const dir = "../../target/criterion";

  const groups = await readdir(dir);
  for (const group of groups) {
    data[group] ||= {};

    const benches = moveToFront(await readdir(`${dir}/${group}`), name => name === 'oxidase');
    for (const bench of benches) {
      data[group][bench] ||= {};

      const parameters = moveToFront(await readdir(`${dir}/${group}/${bench}`), name => name.startsWith('original'));
      for (const parameter of parameters) {
        const json = await import(`${dir}/${group}/${bench}/${parameter}/new/estimates.json`, { with: { type: "json" } });
        const durationMs = json.default.mean.point_estimate / 1_000_000;
        data[group][bench][parameter] ||= durationMs;
      }
    }
  }
  return data
}

const data = await readData();
console.log(formatAsTable(data, (durationMs) => `${durationMs.toFixed(2)} ms`))
