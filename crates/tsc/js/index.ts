import { createProjectSync, ts } from "@ts-morph/bootstrap";

const TS_SOURCE_FILENAME = "a.ts";
export function processTs(
	sourceCode: string,
): {
	ts: string;
	js: string;
	kind: "module" | "script";
} | null {
	sourceCode = sourceCode.replaceAll('/*!', '/*');
    const project = createProjectSync({ useInMemoryFileSystem: true });
    const sourceFile = project.createSourceFile(
        TS_SOURCE_FILENAME,
        sourceCode, {
            scriptKind: ts.ScriptKind.TS,
        }
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

    const program = project.createProgram({ rootNames: [TS_SOURCE_FILENAME], options: compilerOptions });
    // With noCheck enabled, all preEmitDiagnostics are syntax errors
    if (ts.getPreEmitDiagnostics(program).length > 0) {
        return null
    }

	const printer = ts.createPrinter({
		removeComments: true,
	}, {
		substituteNode(_hint, node) {
			if (
				(ts.isEnumDeclaration(node) || ts.isModuleDeclaration(node)) &&
				// Preseve declare enum/namespace
				(ts.getCombinedModifierFlags(node) & ts.ModifierFlags.Ambient) === 0
			) {
				return ts.factory.createNotEmittedStatement(node);
			}
			return node
		}
	});
	const transformedCode = printer.printNode(
		ts.EmitHint.SourceFile,
		sourceFile,
		sourceFile,
	);

	const { outputText } = ts.transpileModule(transformedCode, {
		compilerOptions
	});

	return {
		ts: transformedCode,
		js: outputText,
		kind: ts.isExternalModule(sourceFile) ? "module" : "script",
	};
}

// const JS_SOURCE_FILENAME = "a.js";
// export function formatJs(
// 	sourceCode: string,
// ): string | null {
// 	sourceCode = sourceCode.replaceAll('/*!', '/*');
//     const project = createProjectSync({ useInMemoryFileSystem: true });
//     const sourceFile = project.createSourceFile(
//         JS_SOURCE_FILENAME,
//         sourceCode, {
//             scriptKind: ts.ScriptKind.JS,
//         }
//     );

// 	const compilerOptions: ts.CompilerOptions = {
// 		target: ts.ScriptTarget.Latest,
// 		module: ts.ModuleKind.Preserve,
// 		verbatimModuleSyntax: true,
// 		useDefineForClassFields: true,
// 		removeComments: true,
// 		allowJs: true,
// 		noCheck: true,
// 		noEmit: true,
// 	};

//     const program = project.createProgram({ rootNames: [JS_SOURCE_FILENAME], options: compilerOptions });

//     // With noCheck enabled, all preEmitDiagnostics are syntax errors
//     if (ts.getPreEmitDiagnostics(program).length > 0) {
//         return null
//     }
// 	const printer = ts.createPrinter({
// 		removeComments: true,
// 	});
// 	return printer.printFile(sourceFile)
// }
