// based on https://github.com/oxc-project/bench-javascript-transformer-written-in-rust/blob/e298c6c3be57a4a48176595b5c558dabd98d0288/memory.sh

import { processTs } from 'oxidase_tsc'
import { $ } from 'zx'
import { readdirSync, readFileSync } from 'node:fs'

await $`wasm-pack build --no-opt --release --target web --features wasm`.quiet();

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


for (const file of readdirSync('files')) {
    if (file.startsWith('.')) { // .DS_Store
        continue;
    }
    for (const removeCodegen of [false, true]) {
        let inputPath = `files/${file}`;
        let source: string;
        if (!removeCodegen) {
            const sourceBytes = readFileSync(inputPath);
            const sourceSize = sourceBytes.byteLength;
            source = sourceBytes.toString('utf8');
            console.log(file, "filesize", formatSize(sourceSize));
        } else {
            const sourceWithoutCodegen = processTs(readFileSync(inputPath, 'utf8'), true, true)?.ts;
            if (sourceWithoutCodegen === undefined) {
                throw new Error(`Failed to remove codegen of ${inputPath}`);
            }
            source = sourceWithoutCodegen;
            const sourceSize = Buffer.from(sourceWithoutCodegen, 'utf8').byteLength;
            console.log(`no_codegen_${file}`, "filesize:", formatSize(sourceSize));
        }

        for (const benchee of ["oxidase", "oxc_parser", 'swc_fast_ts_strip'] as const) {
            if (benchee === 'swc_fast_ts_strip' && !removeCodegen) {
                continue;
            }

            const usage = await measureMemory(benchee, source);

            console.log("\t", benchee, formatSize(usage));
        }
        console.log();
    }
}
