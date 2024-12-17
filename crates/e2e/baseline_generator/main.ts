import { Pool, spawn } from 'threads'
import * as path from '@std/path'
import type { Worker as WorkerType } from './worker.ts'
import { walk } from "@std/fs/walk";


const pool = Pool(() => spawn<WorkerType>(new Worker(new URL('./worker.ts', import.meta.url), { type: 'module' })))

const baselineDir = path.resolve('../baseline');

try {
    await Deno.remove(baselineDir, { recursive: true });
} catch {
    // ignored
}
await Deno.mkdir(baselineDir);

const testFilePaths: string[] = [];

for (const walkRoot of ["TypeScript/tests/cases/compiler"]) {
    for await (const dirEntry of walk(path.resolve('..', 'test_repos', walkRoot), { exts: ["ts"] })) {
        testFilePaths.push(dirEntry.path);
    }
}

const results: Array<ReturnType<WorkerType['transpileFile']>> = []

for (const path of testFilePaths) {
    pool.queue(async worker => {
        results.push(await worker.transpileFile(path))
    })
}

setInterval(() => {
    console.log(`{}/{}`, results.length, testFilePaths.length)
}, 500);

await pool.completed()
await pool.terminate()

