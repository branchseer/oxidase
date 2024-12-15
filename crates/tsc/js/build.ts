import * as esbuild from "https://deno.land/x/esbuild@v0.24.0/mod.js";
const result = await esbuild.build({
    bundle: true,
    entryPoints: ["./index.ts"],
    outfile: "./dist.js",
    format: 'iife',
    globalName: 'oxidaseTsc',
    alias: { "fs/promises": "./stub.js"},
    define: { "process.platform": "'linux'" },
    minify: true,
});
console.log("result:", result);
await esbuild.stop();
