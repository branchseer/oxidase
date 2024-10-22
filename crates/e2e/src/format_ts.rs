use crate::common::{remove_empty_statements, ts_syntax};
use oxidase::ModuleKind;
use std::sync::Arc;
use swc::{Compiler, PrintArgs};
use swc_common::{FileName, SourceMap, Spanned};
use swc_ecma_ast::{EsVersion, Program};
use swc_ecma_parser::with_file_parser;

pub fn format_ts(source: impl Into<String>, module_kind: ModuleKind) -> anyhow::Result<String> {
    let cm = Arc::new(SourceMap::default());
    let fm = cm.new_source_file(FileName::Custom("a.js".into()).into(), source.into());
    let mut errors = vec![];

    let mut program = with_file_parser(
        &fm,
        ts_syntax(),
        EsVersion::latest(),
        None,
        &mut errors,
        |parser| {
            Ok(match module_kind {
                ModuleKind::Module => Program::Module(parser.parse_module()?),
                ModuleKind::Script => Program::Script(parser.parse_script()?),
            })
        },
    )
    .map_err(|err| anyhow::anyhow!("{:?}: {}", err.span(), err.kind().msg()))?;

    // if !errors.is_empty() {
    //     anyhow::bail!("{:#?}", errors);
    // }

    remove_empty_statements(&mut program);

    let compiler = Compiler::new(cm);

    let ret = compiler.print(
        &program, // ast to print
        PrintArgs::default(),
    )?;
    Ok(ret.code)
}

#[cfg(test)]
mod tests {
    use crate::format_ts::format_ts;
    use oxidase::ModuleKind;

    #[test]
    fn test_format_ts() {
        assert_eq!(
            format_ts("a = 1;b=2;{ c=3}", ModuleKind::Module).unwrap(),
            "a = 1;\nb = 2;\n{\n    c = 3;\n}"
        );
    }
}
