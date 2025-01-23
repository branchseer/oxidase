import * as esbuild from "esbuild";

const result = await esbuild.build({
    bundle: true,
    entryPoints: ["./index.ts"],
    outfile: "../dist.js",
    format: 'iife',
    globalName: 'oxidaseTsc',
    alias: { "fs/promises": "./stub.js"},
    define: { "process.platform": "'linux'" },
    minify: true,
});
console.log(result);
await esbuild.stop();
