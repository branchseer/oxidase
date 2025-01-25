// https://github.com/oxc-project/bench-javascript-transformer-written-in-rust/blob/e298c6c3be57a4a48176595b5c558dabd98d0288/table.mjs

/**
 * @file table.mjs
 * @description Generate a table from criterion output.
 *
 * Usage:
 *   pnpm table [options]
 *
 * # Options
 *   -f,--format <format>  Output format. 'md' or 'csv'. Default: 'md'
 *   -o,--output <path>    Output file path. Prints to stdout if not set.
 */
import fs from 'node:fs'
import { markdownTable }  from 'markdown-table'

async function readData() {
  const data = {};
  const dir = "../../target/criterion";

  const groups = await fs.promises.readdir(dir);
  for (const group of groups) {
    data[group] ||= {};

    const benches = await fs.promises.readdir(`${dir}/${group}`);
    for (const bench of benches) {
      data[group][bench] ||= {};

      const measurements = await fs.promises.readdir(`${dir}/${group}/${bench}`);
      for (const measurement of measurements) {
        const json = await import(`${dir}/${group}/${bench}/${measurement}/new/estimates.json`, { with: { type: "json" } });
        const duration_ms = json.default.mean.point_estimate / 1_000_000;
        data[group][bench][measurement] ||= { duration_ms };
      }
    }
  }

  return data
}
/**
 * @param {string[]} argv
 */
function parseArgs(argv) {
  const opts = {
    /**
     * output format. Markdown or CSV.
     * @type {'markdown' | 'csv'}
     */
    format: 'markdown',
    /**
     * Path to output file. `null` prints to stdout.
     * @type {string | null}
     */
    output: null,
  };

  for(let arg = argv.shift(); arg; arg = argv.shift()) {
    switch(arg) {
      case '-f':
      case '--format': {

        const format = argv.shift()?.trim()?.toLowerCase();
        if (!format) throw new TypeError('--format flag requires an argument');
        switch(format) {
          case 'md':
          case 'markdown':
            opts.format = 'markdown';
            break;
          case 'csv':
            opts.format = 'csv';
            break;
          default:
            throw new TypeError(`Invalid format '${format}', expected 'md' or 'csv'`);
        }
        break;
      }

      case '-o':
      case '--output': {
        opts.output = argv.shift();
        break;
      }

      // in case someone runs `pnpm table -- --format csv`
      case '--':
        continue
    }
  }

  return opts;
}

async function main(argv) {
  const data = await readData();
  const groups = Object.keys(data);
  const options = parseArgs(argv);

  let out = '';

  switch(options.format) {
    case 'markdown': {
      for (const group of groups) {
        const columns = Object.keys(data[group]);
        const rows = Object.keys(data[group][columns[0]]);
        out += `### ${group}\n`;
        const table = [["", ...columns]];
        for (const row of rows) {
          const column_numbers = columns.map((column) => data[group][column][row].duration_ms);
          const minimum = Math.min(...column_numbers);
          const column_values = column_numbers.map((number) => {
            return `\`${number.toFixed(1)} ms\` (${(number / minimum).toFixed(2)}x)`
          });
          table.push([row, ...column_values]);
        }
        out += markdownTable(table) + '\n';
      }
      break
    }

    case 'csv': {
      const header = ['group', 'bench_name', 'tool', 'measurement', 'duration_ms'];
      out += header.join(',') + '\n';

      for (const group of groups) {
        // swc, oxc
        for (const column of columns) {
          const benches = data[group][column]
          for (const bench in benches) {
            const { duration_ms } = benches[bench];
            out += `${group},${bench},${column},duration,${duration_ms}\n`;
          }
        }
      }
    }
    break;

    default:
      throw new TypeError(`Unexpected output format '${options.format}'`);
  }

  if (!options.output) {
    console.log(out);
  } else {
    await fs.promises.writeFile(options.output, out, 'utf8');
    console.log(`Saved table to ${options.output}`);
  }
}

main(process.argv.slice(2)).catch(console.error);
