import * as esbuild from "esbuild";

const outfile = Deno.args[0]
if (outfile === undefined) {
    console.log('deno task build [outfile]')
    Deno.exit(1);
}

const result = await esbuild.build({
    bundle: true,
    entryPoints: ["./index.ts"],
    outfile: outfile,
    format: 'iife',
    globalName: 'oxidaseTsc',
    alias: { "fs/promises": "./stub.js"},
    define: { "process.platform": "'linux'" },
    minify: true,
});
console.log(result);
await esbuild.stop();
