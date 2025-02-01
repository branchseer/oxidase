use v8_utils::with_isolate;

pub mod v8_utils;

static TSC_JS_SOURCE: &str = include_str!("../dist.js");

pub struct Tsc {
    context: v8::Global<v8::Context>,
    process_ts_func: v8::Global<v8::Function>,
}

#[derive(Debug, serde::Deserialize, PartialEq, Eq)]
pub struct TranspileOutput {
    pub js: String,
    pub ts: String,
    pub kind: SourceKind,
}

#[derive(Debug, serde::Deserialize, serde::Serialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "camelCase")]
pub enum SourceKind {
    Module,
    Script,
}

impl Default for Tsc {
    fn default() -> Self {
        Self::new()
    }
}

impl Tsc {
    pub fn new() -> Self {
        with_isolate(|isolate| {
            let (context, process_ts_func) = {
                let handle_scope = &mut v8::HandleScope::new(isolate);

                let context = v8::Context::new(handle_scope, Default::default());
                let scope = &mut v8::ContextScope::new(handle_scope, context);
                let code = v8::String::new(scope, TSC_JS_SOURCE).unwrap();
                let script = v8::Script::compile(scope, code, None).unwrap();
                script.run(scope).unwrap();

                let oxidase_tsc_name = v8::String::new(scope, "oxidaseTsc").unwrap().cast();
                let oxidase_tsc = context
                    .global(scope)
                    .get(scope, oxidase_tsc_name)
                    .unwrap()
                    .cast::<v8::Object>();

                let process_ts_name = v8::String::new(scope, "processTs").unwrap().cast();
                let process_ts_func = oxidase_tsc
                    .get(scope, process_ts_name)
                    .unwrap()
                    .cast::<v8::Function>();

                (
                    v8::Global::new(scope, context),
                    v8::Global::new(scope, process_ts_func),
                )
            };

            Self {
                // isolate,
                context,
                process_ts_func,
            }
        })
    }

    pub fn process_ts(
        &mut self,
        source: &str,
        strip_enum_and_namespace: bool,
        strip_parameters_with_modifiers: bool,
    ) -> Option<TranspileOutput> {
        with_isolate(|isolate| {
            let process_ts_func = self.process_ts_func.open(isolate);
            let context = self.context.open(isolate);
            let handle_scope = &mut v8::HandleScope::with_context(isolate, &self.context);
            let global = context.global(handle_scope);

            let source = v8::String::new(handle_scope, source)?;
            let strip_enum_and_namespace = v8::Boolean::new(handle_scope, strip_enum_and_namespace);
            let strip_parameters_with_modifiers =
                v8::Boolean::new(handle_scope, strip_parameters_with_modifiers);

            let result = process_ts_func.call(
                handle_scope,
                global.cast(),
                &[
                    source.cast(),
                    strip_enum_and_namespace.cast(),
                    strip_parameters_with_modifiers.cast(),
                ],
            )?;
            serde_v8::from_v8::<Option<TranspileOutput>>(handle_scope, result).ok()?
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_syntax() {
        let mut tsc = Tsc::new();

        assert_eq!(tsc.process_ts("let a: string =", true, false), None);
    }

    #[test]
    fn script_kind() {
        let mut tsc = Tsc::new();
        assert_eq!(
            tsc.process_ts("let a: string = 1", true, false).unwrap().kind,
            SourceKind::Script
        );
    }
    #[test]
    fn module_kind() {
        let mut tsc = Tsc::new();
        assert_eq!(
            tsc.process_ts("export let a: string = 1", true, false)
                .unwrap()
                .kind,
            SourceKind::Module
        );
    }
}
