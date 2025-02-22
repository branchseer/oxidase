import { transpile } from '../wasm_node/oxidase_wasm_bindings.js'

export async function resolve(specifier, context, nextResolve) {
    try {
        return await nextResolve(specifier, context);
    } catch (err) {
        if (err.url?.endsWith(".js")) {
            return nextResolve(err.url.slice(0, -".js".length) + ".ts");
        }
        if (err.url?.endsWith(".mjs")) {
            return nextResolve(err.url.slice(0, -".mjs".length) + ".mts");
        }
        throw err;
    }
}

export async function load(url, context, nextLoad) {
    if (!url.endsWith(".ts") && !url.endsWith(".mts")) {
        return nextLoad(url, context);
    }

    const format = "module";
    const nextLoadResult = await nextLoad(url, { ...context, format });
    const output = transpile(nextLoadResult.source.toString(), url);

    return {
        format,
        shortCircuit: true,
        source: output + "\n//# sourceURL=" + url,
    };
}
