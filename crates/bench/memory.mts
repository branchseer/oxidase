// based on https://github.com/oxc-project/bench-javascript-transformer-written-in-rust/blob/e298c6c3be57a4a48176595b5c558dabd98d0288/memory.sh

import { processTs } from 'oxidase_tsc'
import { $ } from 'zx'
import { readdirSync, readFileSync, writeFileSync } from 'node:fs'

await $`cargo build --release --bin memory`.quiet()

function formatSize(bytes: number) {
    return `${(bytes / 1024 / 1024).toPrecision(3)} MB`
}

for (const file of readdirSync('files')) {
    if (file.startsWith('.')) { // .DS_Store
        continue;
    }
    for (const removeCodegen of [false, true]) {
        let inputPath = `files/${file}`;
        if (!removeCodegen) {
            const sourceSize = readFileSync(inputPath).byteLength;
            console.log(file, "filesize", formatSize(sourceSize));
        } else {
            const sourceWithoutCodegen = processTs(readFileSync(inputPath, 'utf8'), true, true)?.ts;
            if (sourceWithoutCodegen === undefined) {
                throw new Error(`Failed to remove codegen of ${inputPath}`);
            }
            const sourceWithoutCodegenBytes = Buffer.from(sourceWithoutCodegen, 'utf8');
            inputPath = `../../target/no_codegen_${file}`;
            writeFileSync(inputPath, sourceWithoutCodegenBytes);

            const sourceSize = sourceWithoutCodegenBytes.byteLength;
            console.log(`no_codegen_${file}`, "filesize:", formatSize(sourceSize));
        }

        for (const benchee of ["oxidase", "oxc_parser", 'swc_fast_ts_strip']) {
            if (benchee === 'swc_fast_ts_strip' && !removeCodegen) {
                continue;
            }
            const outputLines = await $`hyperfine --warmup 10 --show-output "/usr/bin/time -al ../../target/release/memory ${benchee} ${inputPath} > /dev/null"`.quiet().lines();
            let total = 0;
            let count = 0;
            for (let line of outputLines) {
                line = line.trim();
                if (!line.endsWith('maximum resident set size')) {
                    continue;
                }
                const size = line.split(/(\s+)/)[0];
                total += parseInt(size.trim());
                count += 1;
            }
            console.log("\t", benchee, formatSize(total / count));
        }
        console.log();
    }
}
