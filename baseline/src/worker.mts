import { isMainThread, worker } from "workerpool";
import * as ts from "typescript";
import { readFileSync, writeFileSync } from "node:fs";

import { getPath } from "@dprint/typescript";
import { createFromBuffer } from "@dprint/formatter";
import { transpilePath } from "./paths.mjs";

const buffer = readFileSync(getPath());
const formatter = createFromBuffer(buffer);

const compilerHost = ts.createCompilerHost({});

function containsJSX(node: ts.Node): boolean {
	return !!ts.forEachChild(node, (child) => {
		if (
			ts.isJsxElement(node) ||
			ts.isJsxSelfClosingElement(node) ||
			containsJSX(child)
		) {
			return true;
		}
		return undefined;
	});
}

export function checkJs(
	source: string,
	kind: "module" | "script" | null,
): "module" | "script" | null {
	let impliedNodeFormat: ts.ResolutionMode = undefined;
	switch (kind) {
		case "module": {
			impliedNodeFormat = ts.ModuleKind.ESNext;
			break;
		}
		case "script": {
			impliedNodeFormat = ts.ModuleKind.CommonJS;
			break;
		}
	}
	const compilerOptions: ts.CompilerOptions = {
		alwaysStrict: false,
		target: ts.ScriptTarget.ESNext,
		allowJs: true,
		noLib: true,
		skipLibCheck: true,
		noResolve: true,
		noEmit: true,
		noCheck: true,
		module: ts.ModuleKind.Preserve,
		jsx: ts.JsxEmit.None,
	};
	if (kind === "module") {
		// compilerOptions.module = ts.ModuleKind.ESNext;
		compilerOptions.moduleDetection = ts.ModuleDetectionKind.Force;
	}
	compilerHost.getSourceFile = (
		fileName: string,
		languageVersion: ts.ScriptTarget,
	) => {
		if (fileName !== "a.js") {
			return undefined;
		}
		const sourceFile = ts.createSourceFile(
			fileName,
			source,
			{ languageVersion, impliedNodeFormat },
			true,
			ts.ScriptKind.JS,
		);
		sourceFile.languageVariant = ts.LanguageVariant.Standard;
		return sourceFile;
	};
	const program = ts.createProgram(["a.js"], compilerOptions, compilerHost);

	const sourceFile = program.getSourceFile("a.js")!;
	console.log(sourceFile.languageVariant);

	if (
		ts
			.getPreEmitDiagnostics(program, sourceFile)
			.some((source) => source.file !== undefined)
	) {
		return null;
	}

	// sourceFile

	if (containsJSX(sourceFile)) {
		return null;
	}
	if (kind !== null) {
		return kind;
	}
	return ts.isExternalModule(program.getSourceFile("a.js")!)
		? "module"
		: "script";
}

function transformer<T extends ts.Node>(context: ts.TransformationContext) {
	return (rootNode: T): ts.Node => {
		function visit(node: ts.Node): ts.Node {
			if (
				(ts.isEnumDeclaration(node) || ts.isModuleDeclaration(node)) &&
				// Preseve declare enum/namespace
				(ts.getCombinedModifierFlags(node) & ts.ModifierFlags.Ambient) === 0
			) {
				return context.factory.createNotEmittedStatement(node);
			}
			return ts.visitEachChild(node, visit, context);
		}
		return ts.visitNode(rootNode, visit);
	};
}

export function processTs(
	sourceCode: string,
	kind: "module" | "script" | null,
): {
	ts: string;
	js: string;
	kind: "module" | "script";
} | null {
	let impliedNodeFormat: ts.ResolutionMode = undefined;
	switch (kind) {
		case "module": {
			impliedNodeFormat = ts.ModuleKind.ESNext;
			break;
		}
		case "script": {
			impliedNodeFormat = ts.ModuleKind.CommonJS;
			break;
		}
	}
	const sourceFile = ts.createSourceFile(
		"a.ts",
		sourceCode,
		{ languageVersion: ts.ScriptTarget.Latest, impliedNodeFormat },
		true,
		ts.ScriptKind.TS,
	);

	const result = ts.transform(sourceFile, [transformer]);
	const transformedSourceFile = result.transformed[0]!;

	const printer = ts.createPrinter();
	const transformedCode = printer.printNode(
		ts.EmitHint.Unspecified,
		transformedSourceFile,
		sourceFile,
	);
	const compilerOptions: ts.CompilerOptions = {
		target: ts.ScriptTarget.ESNext,
		module: ts.ModuleKind.Preserve,
		verbatimModuleSyntax: true,
		useDefineForClassFields: true,
		removeComments: false,
		noCheck: true,
	};

	const { outputText, diagnostics } = ts.transpileModule(transformedCode, {
		compilerOptions,
		reportDiagnostics: true,
	});

	if (diagnostics !== undefined && diagnostics.length > 0) {
		return null;
	}

	return {
		ts: transformedCode,
		js: formatter.formatText({ filePath: "a.ts", fileText: outputText }),
		kind: (kind ?? ts.isExternalModule(sourceFile)) ? "module" : "script",
	};
}

export type CheckJsResult = {
	id: number;
	path: string;
	kind: "script" | "module";
} | null;

export interface TsProcessResult {
	id: number;
	path: string;
	kind: "module" | "script";
}

const workerMethods = {
	checkJs: (id: number, path: string, isModule: boolean): CheckJsResult => {
		try {
			const source = readFileSync(path, "utf8");
			const kind = checkJs(source, isModule ? "module" : null);
			if (kind === null) {
				return null;
			}
			writeFileSync(`${transpilePath}/${id}`, source);
			return {
				id,
				path,
				kind,
			};
		} catch {
			return null;
		}
	},
	processTs: (
		id: number,
		path: string,
		isModule: boolean,
	): TsProcessResult | null => {
		try {
			const source = readFileSync(path, "utf8");
			const processResult = processTs(source, isModule ? "module" : null);
			if (processResult === null) {
				return null;
			}
			writeFileSync(`${transpilePath}/${id}.input`, processResult.ts);
			writeFileSync(`${transpilePath}/${id}.output`, processResult.js);

			return { id, path, kind: processResult.kind };
		} catch {
			return null;
		}
	},
};

export type WorkerMethods = typeof workerMethods;

if (!isMainThread) {
	worker(workerMethods);
}
