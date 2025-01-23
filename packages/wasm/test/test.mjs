import test from 'node:test';
import assert from 'node:assert/strict'
import { transpile } from 'oxidase';
import { $ } from 'zx';
import { fileURLToPath } from 'node:url';
import path from 'node:path';
const dirname = path.dirname(fileURLToPath(import.meta.url));


test('api', async () => {
    const output = transpile('a.ts', "let a: number = 1");
    assert.equal(output, 'let a         = 1');
})

test('loader', async () => {
    const { stdout } = await $({ cwd: dirname })`${process.execPath} --import oxidase/register fixture.ts`;
    assert.equal(stdout, "1\n");
})
