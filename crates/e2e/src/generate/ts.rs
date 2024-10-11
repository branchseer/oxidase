use crate::common::{remove_empty_statements, ts_syntax};
use oxidase::ModuleKind;
use rayon::spawn;
use std::ops::Range;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::Path;
use swc::{Compiler, PrintArgs};
use swc_common::source_map::SmallPos;
use swc_common::{source_map::SourceMap, sync::Lrc, BytePos, Mark, Span, GLOBALS};
use swc_ecma_ast::{Decl, EsVersion, Program, TsExportAssignment};
use swc_ecma_parser::{parse_file_as_program, Syntax, TsSyntax};
use swc_ecma_transforms_typescript::typescript::{typescript, Config};
use swc_ecma_transforms_typescript::{ImportsNotUsedAsValues, TsImportExportAssignConfig};
use swc_ecma_visit::{FoldWith, Visit, VisitMut, VisitMutWith};

// enum InsertPointKind {
//     Declare,
//     Semicolon,
// }

struct Patch {
    content: &'static str,
    span: Span,
}

#[derive(Default)]
struct PreprocessVisitor {
    patches: Vec<Patch>,
    export_assignment_span: Option<Span>,
}

impl VisitMut for PreprocessVisitor {
    fn visit_mut_decl(&mut self, decl: &mut Decl) {
        match decl {
            Decl::TsEnum(enum_decl) => {
                if !enum_decl.declare {
                    enum_decl.declare = true;
                    self.patches.push(Patch {
                        span: enum_decl.span,
                        content: ";",
                    })
                }
            }
            Decl::TsModule(module_decl) => {
                if !module_decl.declare {
                    module_decl.declare = true;
                    self.patches.push(Patch {
                        span: module_decl.span,
                        content: ";",
                    })
                }
            }
            _ => {
                decl.visit_mut_children_with(self);
            }
        }
    }
    fn visit_mut_ts_export_assignment(&mut self, node: &mut TsExportAssignment) {
        // let range = to_range(node.span);
        self.patches.push(Patch {
            span: node.span,
            content: ";",
        });
        if self.export_assignment_span.is_none() {
            self.export_assignment_span = Some(node.span);
        }
    }
}

pub struct TsTranspileReturn {
    pub code: String,
    pub module_kind: ModuleKind,
}

fn to_byte_range(span: Span, source_map: &SourceMap) -> Range<usize> {
    let start = source_map.lookup_byte_offset(span.lo).pos.0;
    let end = source_map.lookup_byte_offset(span.hi).pos.0;
    start as usize..end as usize
}

const BOM: &str = "\u{feff}";
pub fn transpile_ts(path: &Path, ts_code: &mut String) -> Option<TsTranspileReturn> {
    if ts_code.starts_with(BOM) {
        ts_code.truncate(BOM.len());
    }
    GLOBALS.set(&Default::default(), || {
        let ts_code = ts_code;
        let cm = Lrc::new(SourceMap::new(swc_common::FilePathMapping::empty()));

        let compiler = Compiler::new(cm.clone());

        let source = cm.new_source_file(
            swc_common::FileName::Custom("a.ts".into()).into(),
            ts_code.clone(),
        );
        let mut errors = vec![];

        let mut program =
            parse_file_as_program(&source, ts_syntax(), EsVersion::latest(), None, &mut errors)
                .ok()?;

        let module_kind = match &program {
            Program::Module(_) => ModuleKind::Module,
            Program::Script(_) => ModuleKind::Script,
        };

        let mut preprocess_visitor = PreprocessVisitor::default();
        program.visit_mut_with(&mut preprocess_visitor);

        let mut program = program.fold_with(&mut typescript(
            Config {
                verbatim_module_syntax: true,
                native_class_properties: true,
                import_not_used_as_values: ImportsNotUsedAsValues::Preserve,
                no_empty_export: true,
                import_export_assign_config: TsImportExportAssignConfig::Classic,
                ts_enum_is_mutable: false,
            },
            Mark::new(),
            Mark::new(),
        ));

        let export_assignment = preprocess_visitor
            .export_assignment_span
            .map(|span| ts_code[to_byte_range(span, &cm)].to_owned());

        // let mut start = 0usize;
        // for patch in preprocess_visitor.patches {
        //     if !ts_code.is_char_boundary(patch.span.lo.0 as usize)
        //         || !ts_code.is_char_boundary(patch.span.hi.0 as usize)
        //     {
        //         // TODO: happens in TypeScript@89e004f632323a276b67649e118e78f39a7dc429
        //         // tests/cases/compiler/constIndexedAccess.ts
        //         // why is this possible?
        //         return None;
        //     }
        // }

        for patch in preprocess_visitor.patches.into_iter().rev() {
            let range = to_byte_range(patch.span, &cm);
            // if !ts_code.is_char_boundary(range.start) || !ts_code.is_char_boundary(range.end) {
            //     // TODO: happens in TypeScript@89e004f632323a276b67649e118e78f39a7dc429
            //     // tests/cases/compiler/constIndexedAccess.ts
            //     // why is this possible?
            //     return None;
            // }
            ts_code.replace_range(range, &patch.content);
            // ts_code.insert_str(declare_insert_points.to_usize() - 1, "declare ")
        }

        if let Some(export_assignment) = export_assignment {
            ts_code.push_str("\n;");
            ts_code.push_str(&export_assignment);
        }

        remove_empty_statements(&mut program);

        let ret = compiler
            .print(
                &program, // ast to print
                PrintArgs::default(),
            )
            .ok()?;

        Some(TsTranspileReturn {
            code: ret.code,
            module_kind,
        })
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use pretty_assertions::assert_eq;
    #[test]
    fn detect_module() {
        let ret = transpile_ts(
            Path::new(""),
            &mut String::from("export const a: string = ''"),
        )
        .unwrap();
        assert_eq!(ret.module_kind, ModuleKind::Module);
    }
    #[test]
    fn rejects_invalid() {
        assert!(transpile_ts(Path::new(""), &mut String::from("a + ")).is_none());
    }
    #[test]
    fn rejects_jsx() {
        assert!(transpile_ts(Path::new(""), &mut String::from("a = <div />")).is_none());
    }

    #[test]
    fn preprocess_enum_namespace_exports() {
        let mut source = String::from("enum A {}\nfoo;export = 1\nbar;\nnamespace B {;}");
        let ret = transpile_ts(Path::new(""), &mut source).unwrap();
        assert_eq!(
            source,
            "declare enum A {}\nfoo;;\nbar;\ndeclare namespace B {;};export = 1"
        );
        assert_eq!(ret.code, "foo;\nbar;\nmodule.exports = 1;\n");
    }
    #[test]
    fn hello() {
        let mut source = String::from(
            r#"module Z.M {
    export function bar() {
        return "";
    }
}
module A.M {
    import M = Z.M;
    export function bar() {
    }
    M.bar(); // Should call Z.M.bar
}"#,
        );
        let mut source = std::fs::read_to_string("/Users/patr0nus/code/oxidase/crates/e2e/test_repos/TypeScript/tests/cases/compiler/recursiveExportAssignmentAndFindAliasedType7.ts").unwrap();

        let source_before = source.clone();
        let ret = transpile_ts(Path::new(""), &mut source).unwrap();
        eprintln!("{}", source);
    }
    #[test]
    fn preserve_imports() {
        let mut source = String::from(
            "import { a } from 'a'; import type b from 'b'; export { A }; export type { B };",
        );
        let source_before = source.clone();
        let ret = transpile_ts(Path::new(""), &mut source).unwrap();
        assert_eq!(ret.code, "import { a } from 'a';\nexport { A };\n");
    }

    #[test]
    fn strip_types() {
        let mut source = String::from(
            "\n class a { @zz s: string\n constructor(private b: string = 231) { b?.c?.(); } }",
        );
        let source_before = source.clone();
        let ret = transpile_ts(Path::new(""), &mut source).unwrap();
        assert_eq!(source, source_before);
        assert_eq!(ret.module_kind, ModuleKind::Script);
        assert_eq!(
            ret.code,
            r#"class a {
    b;
    @zz
    s;
    constructor(b = 231){
        this.b = b;
        b?.c?.();
    }
}
"#
        )
    }
}
