import { processTs } from 'oxidase_tsc'
import { $ } from 'zx'
import { readFileSync } from 'node:fs'
import { type Data, readdir, formatAsTable } from './utils.mjs';

await $`wasm-pack build --no-opt --release --target web --features wasm`;

if (process.argv.includes('--no-run')) {
    process.exit(0);
}

const wasmBinary = readFileSync('./pkg/oxidase_bench_bg.wasm');
const wasmModule = await WebAssembly.compile(wasmBinary);

let i = 0;
async function measureMemory(bencheeName: "oxidase" | "oxc_parser" | 'swc_fast_ts_strip', source: string) {
    //  https://futurestud.io/tutorials/node-js-esm-bypass-cache-for-dynamic-imports
    const { initSync, measure_memory, Benchee } = await import(`./pkg/oxidase_bench.js?${++i}`) as typeof import('./pkg/oxidase_bench.js');
    initSync({ module: wasmModule });
    return measure_memory(({
        'oxidase': Benchee.Oxidase,
        'oxc_parser': Benchee.OxcParser,
        'swc_fast_ts_strip': Benchee.SwcFastTsStrip,
    })[bencheeName], source);
}

function formatSize(bytes: number) {
    return `${(bytes / 1024 / 1024).toFixed(3)} MB`
}

const data: Data = {}

for (const file of await readdir('files')) {
    for (const erasableSyntaxOnly of [false, true]) {
        let inputPath = `files/${file}`;
        let source: string;
        let parameter: string;
        if (!erasableSyntaxOnly) {
            const sourceBytes = readFileSync(inputPath);
            const sourceSize = sourceBytes.byteLength;
            source = sourceBytes.toString('utf8');
            parameter = `original (${formatSize(sourceSize)})`;
        } else {
            const sourceWithErasableSyntaxOnly = processTs(readFileSync(inputPath, 'utf8'), true, true)?.ts;
            if (sourceWithErasableSyntaxOnly === undefined) {
                throw new Error(`Failed to remove non-erasable syntax of ${inputPath}`);
            }
            source = sourceWithErasableSyntaxOnly;
            const sourceSize = Buffer.from(sourceWithErasableSyntaxOnly, 'utf8').byteLength;
            parameter = `erasable syntax only (${formatSize(sourceSize)})`;
        }

        for (const benchee of ["oxidase", "oxc_parser", 'swc_fast_ts_strip'] as const) {
            if (benchee === 'swc_fast_ts_strip' && !erasableSyntaxOnly) {
                continue;
            }

            const usage = await measureMemory(benchee, source);

            data[file] ||= {};
            data[file][benchee] ||= {};
            data[file][benchee][parameter] = usage;
        }
    }
}

console.log(formatAsTable(data, formatSize));
