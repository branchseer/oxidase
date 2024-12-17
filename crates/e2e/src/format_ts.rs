use crate::common::remove_empty_statements;
use std::sync::Arc;
use swc::{Compiler, PrintArgs};
use swc_common::{FileName, SourceMap, Spanned};
use swc_ecma_ast::{EsVersion, Program};
use swc_ecma_parser::{with_file_parser, Syntax};

pub fn format_js(source: impl Into<String>) -> anyhow::Result<String> {
    let cm = Arc::new(SourceMap::default());
    let fm = cm.new_source_file(FileName::Custom("a.js".into()).into(), source.into());
    let mut errors = vec![];

    let mut program = with_file_parser(
        &fm,
        Syntax::Es(swc_ecma_parser::EsSyntax {
            decorators: true,
            import_attributes: true,
            allow_return_outside_function: true,
            ..Default::default()
        }),
        EsVersion::latest(),
        None,
        &mut errors,
        |parser| Ok(Program::Module(parser.parse_module()?)),
    )
    .map_err(|err| anyhow::anyhow!("{:?}: {}", err.span(), err.kind().msg()))?;

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
    use super::*;

    #[test]
    fn test_format_ts() {
        assert_eq!(
            format_js(r#"const alias = require('foo')"#).unwrap(),
            "const alias = require(\"foo\");\n"
        );
        assert_eq!(
            format_js(r#"var let = 1; await 2"#).unwrap(),
            "var let = 1;\nawait 2;\n"
        );
    }

    #[test]
    fn hello() {
        assert_eq!(
            format_js(
                r#" class B  {
    
    
    
     get readonlyProp(): string{}
     set readonlyProp(val: string){}
    
}
class C extends B {
    get prop() { return "foo"; }
    set prop(v) { }
    raw = "edge";
     ro = "readonly please";
    readonlyProp;
    m() { }
}"#
            )
            .unwrap(),
            "a.a = 1;\n"
        );
    }
}
