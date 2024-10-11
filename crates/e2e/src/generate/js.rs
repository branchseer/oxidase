use oxidase::ModuleKind;
use swc_common::{FileName, SourceMap};
use swc_ecma_ast::{EsVersion, Program};
use swc_ecma_parser::{parse_file_as_program, EsSyntax, Syntax};

pub fn detect_js_kind(source: impl Into<String>) -> Option<ModuleKind> {
    let cm: SourceMap = Default::default();
    let fm = cm.new_source_file(FileName::Custom("a.js".into()).into(), source.into());
    let mut errors = vec![];
    let program = parse_file_as_program(
        &fm,
        Syntax::Es(EsSyntax {
            decorators: true,
            export_default_from: true,
            import_attributes: true,
            allow_return_outside_function: true,
            ..Default::default()
        }),
        EsVersion::latest(),
        None,
        &mut errors,
    )
    .ok()?;
    if !errors.is_empty() {
        return None;
    }
    Some(match program {
        Program::Module(_) => ModuleKind::Module,
        Program::Script(_) => ModuleKind::Script,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_js_kind_script() {
        assert_eq!(detect_js_kind("var let = 1"), Some(ModuleKind::Script));
    }
    #[test]
    fn detect_js_kind_module() {
        assert_eq!(detect_js_kind("export {}"), Some(ModuleKind::Module));
    }
    #[test]
    fn detect_js_kind_reject_invalid() {
        assert_eq!(detect_js_kind("a +"), None);
    }
    #[test]
    fn detect_js_kind_reject_jsx() {
        assert_eq!(detect_js_kind("a = <div />"), None);
    }
}
