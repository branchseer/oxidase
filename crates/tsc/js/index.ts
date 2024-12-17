import { createProjectSync, ts } from "@ts-morph/bootstrap";

const TS_SOURCE_FILENAME = "a.ts";
export function processTs(
	sourceCode: string,
): {
	ts: string;
	js: string;
	kind: "module" | "script";
} | null {
	sourceCode = sourceCode.replaceAll("/*!", "/*");
	const project = createProjectSync({ useInMemoryFileSystem: true });
	const sourceFile = project.createSourceFile(
		TS_SOURCE_FILENAME,
		sourceCode,
		{
			scriptKind: ts.ScriptKind.TS,
		},
	);

	const compilerOptions: ts.CompilerOptions = {
		target: ts.ScriptTarget.Latest,
		module: ts.ModuleKind.Preserve,
		verbatimModuleSyntax: true,
		useDefineForClassFields: true,
		removeComments: true,
		noCheck: true,
		noEmit: true,
	};

	const program = project.createProgram({
		rootNames: [TS_SOURCE_FILENAME],
		options: compilerOptions,
	});
	// With noCheck enabled, all preEmitDiagnostics are syntax errors
	if (ts.getPreEmitDiagnostics(program).length > 0) {
		return null;
	}

	const printer = ts.createPrinter({
		removeComments: true,
	}, {
		substituteNode(_hint, node) {
			// ## workarounds for inconsitent behaviors between tsc and oxidase

			// ## `get a();` / `constructor();`
			// - tsc generates body for them
			// - oxidase strips them (TODO: to be consistent with tsc)
			if (ts.isAccessor(node) && (node.body === undefined)) {
				return ts.factory.createNotEmittedStatement(node);
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
				return ts.factory.createNotEmittedStatement(node);
			}

			// ## Codegen node (enum and namespace)
			if (
				(ts.isEnumDeclaration(node) || ts.isModuleDeclaration(node)) &&
				// Preseve declare enum/namespace
				(ts.getCombinedModifierFlags(node) & ts.ModifierFlags.Ambient) === 0
			) {
				return ts.factory.createNotEmittedStatement(node);
			}
			return node;
		},
	});
	const transformedCode = printer.printNode(
		ts.EmitHint.SourceFile,
		sourceFile,
		sourceFile,
	);

	const { outputText } = ts.transpileModule(transformedCode, {
		compilerOptions,
	});

	return {
		ts: transformedCode,
		js: outputText,
		kind: ts.isExternalModule(sourceFile) ? "module" : "script",
	};
}
