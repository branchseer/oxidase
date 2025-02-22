import { createProjectSync, ts } from "@ts-morph/bootstrap";

const TS_SOURCE_FILENAME = "a.ts";

const compilerOptions: ts.CompilerOptions = {
	target: ts.ScriptTarget.Latest,
	module: ts.ModuleKind.Preserve,
	verbatimModuleSyntax: true,
	useDefineForClassFields: true,
	removeComments: true,
	noCheck: true,
	noEmit: true,
	noResolve: true,
	isolatedModules: true,
};

export function processTs(
	sourceCode: string,
	stripEnumAndNamespace?: boolean,
	stripParametersWithModifiers?: boolean,
): {
	ts: string;
	js: string;
	kind: "module" | "script";
} | null {
	sourceCode = sourceCode.replaceAll("/*!", "/*").replaceAll('/// <reference', '//');
	const project = createProjectSync({ useInMemoryFileSystem: true });
	const sourceFile = project.createSourceFile(
		TS_SOURCE_FILENAME,
		sourceCode,
		{
			scriptKind: ts.ScriptKind.TS,
		},
	);
	const program = project.createProgram({
		rootNames: [TS_SOURCE_FILENAME],
		options: compilerOptions,
	});
	// With noCheck enabled, all preEmitDiagnostics are syntax errors
	if (ts.getPreEmitDiagnostics(program).length > 0) {
		return null;
	}


	function shouldRemove(node: ts.Node): boolean {
		//  workarounds for inconsitent behaviors between tsc and oxidase

		// `get a();` / `constructor();`
		// - tsc generates body for them
		// - oxidase strips them (TODO: to be consistent with tsc)
		if (ts.isAccessor(node) && (node.body === undefined)) {
			return true;
		}

		// `export import foo = require('foo')`
		// - tsc generates `const foo = require('foo'); export { foo };`
		// - oxidase generates `export const foo = require('foo');`
		if (
			ts.isImportEqualsDeclaration(node) && !node.isTypeOnly &&
			node.modifiers?.some((modifier) =>
				modifier.kind === ts.SyntaxKind.ExportKeyword
			) && ts.isExternalModuleReference(node.moduleReference)
		) {
			return true;
		}


		//  Codegen node (enum and namespace)
		if (stripEnumAndNamespace && (
			(ts.isEnumDeclaration(node) || ts.isModuleDeclaration(node)) &&
			// Preseve declare enum/namespace
			(ts.getCombinedModifierFlags(node) & ts.ModifierFlags.Ambient) === 0
		)) {
			return true;
		}
		return false;
	}

	const patches: [number, number, string][] = [];
	function visit(node: ts.Node) {
		if (shouldRemove(node)) {
			patches.push([node.pos, node.end, ';']);
			return;
		}
		if (stripParametersWithModifiers && ts.isParameter(node) && node.modifiers && node.modifiers.length > 0) {
			patches.push([node.modifiers.pos, node.modifiers.end, '']);
		}
		ts.forEachChild(node, visit);
	}
	visit(sourceFile);

	let start = 0;
	let codeSegments: string[] = [];
	if (patches.length > 0) {
		for (const [patchStart, patchEnd, replacement] of patches) {
			codeSegments.push(sourceCode.slice(start, patchStart));
			codeSegments.push(replacement);
			start = patchEnd;
		}
		codeSegments.push(sourceCode.slice(start));
		sourceCode = codeSegments.join('');
	}

	const { outputText } = ts.transpileModule(sourceCode, { compilerOptions });

	return {
		ts: sourceCode,
		js: outputText,
		kind: ts.isExternalModule(sourceFile) ? "module" : "script",
	};
}
