import * as workerpool from "workerpool";

import fs from "node:fs";
import path from "node:path";
import { Writable } from "node:stream";
import { stringify } from "csv-stringify/sync";

import type {
	CheckJsResult,
	TsProcessResult,
	WorkerMethods,
} from "./worker.mjs";
import { transpilePath } from "./paths.mjs";

const pool = workerpool.pool(`${import.meta.dirname}/worker.js`, {
	workerType: "thread",
});

const poolWorker = await pool.proxy<WorkerMethods>();

function isPackageTypeModule(dir: string): boolean {
	try {
		const packageJson = fs.readFileSync(`${dir}/package.json`, "utf8");
		return JSON.parse(packageJson).type === "module";
	} catch {
		// ignored
	}
	return false;
}

let lastId = 0;

const checkJsTasks: PromiseLike<CheckJsResult>[] = [];
const processTsTasks: PromiseLike<TsProcessResult>[] = [];

fs.rmSync(transpilePath, { recursive: true, force: true });
fs.mkdirSync(transpilePath);

// Function to recursively walk a folder in parallel
function walk(directory: string, isParentModule: boolean) {
	const entries = fs.readdirSync(directory, { withFileTypes: true });

	const isModule = isParentModule || isPackageTypeModule(directory);

	const subfolders: string[] = [];

	for (const entry of entries) {
		const fullPath = `${directory}/${entry.name}`;

		if (entry.isDirectory()) {
			subfolders.push(fullPath);
		} else if (entry.isFile()) {
			let isFileModule: boolean = isModule;
			let lang: "js" | "ts";

			if (entry.name.endsWith(".js")) {
				lang = "js";
			} else if (entry.name.endsWith(".ts") && !entry.name.endsWith(".d.ts")) {
				lang = "ts";
			} else if (entry.name.endsWith(".mjs")) {
				lang = "js";
				isFileModule = true;
			} else if (
				entry.name.endsWith(".mts") &&
				!entry.name.endsWith(".d.mts")
			) {
				lang = "ts";
				isFileModule = true;
			} else if (entry.name.endsWith(".cjs")) {
				lang = "js";
				isFileModule = false;
			} else if (
				entry.name.endsWith(".cts") &&
				!entry.name.endsWith(".d.cts")
			) {
				lang = "ts";
				isFileModule = false;
			} else {
				continue;
			}
			const id = lastId++;
			switch (lang) {
				case "js": {
					checkJsTasks.push(poolWorker.checkJs(id, fullPath, isFileModule));
					break;
				}
				case "ts": {
					processTsTasks.push(poolWorker.processTs(id, fullPath, isFileModule));
					break;
				}
			}
		}
	}
	for (const subfolder of subfolders) {
		walk(subfolder, isModule);
	}
}

const transpileSourcePath = path.resolve(
	import.meta.dirname,
	"../sources/transpile",
);

walk(transpileSourcePath, false);

const fileList = Writable.toWeb(
	fs.createWriteStream(`${transpilePath}/_list.csv`, "utf8"),
);

const fileListWriter = fileList.getWriter();

for (const checkJsResult of await Promise.all(checkJsTasks)) {
	if (checkJsResult === null) {
		continue;
	}
	fileListWriter.write(
		stringify([
			[
				checkJsResult.id,
				checkJsResult.path.slice(transpileSourcePath.length + 1),
				"js",
				checkJsResult.kind,
			],
		]),
	);
}
for (const processTsResult of await Promise.all(processTsTasks)) {
	if (processTsResult === null) {
		continue;
	}
	fileListWriter.write(
		stringify([
			[
				processTsResult.id,
				processTsResult.path.slice(transpileSourcePath.length + 1),
				"ts",
				processTsResult.kind,
			],
		]),
	);
}

await fileListWriter.close();

await pool.terminate();
