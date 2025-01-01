use std::sync::Once;

static TSC_JS_SOURCE: &str = include_str!(concat!(env!("OUT_DIR"), "/dist.js"));

pub struct Tsc {
    isolate: v8::OwnedIsolate,
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

impl Tsc {
    pub fn new() -> Self {

        let mut isolate = v8::Isolate::new(v8::CreateParams::default());

        let (context, process_ts_func) = {
            let handle_scope = &mut v8::HandleScope::new(&mut isolate);

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
            isolate,
            context,
            process_ts_func,
        }
    }

    pub fn process_ts(&mut self, source: &str, strip_enum_and_namespace: bool) -> Option<TranspileOutput> {
        let process_ts_func = self.process_ts_func.open(&mut self.isolate);
        let context = self.context.open(&mut self.isolate);
        let handle_scope = &mut v8::HandleScope::with_context(&mut self.isolate, &self.context);
        let global = context.global(handle_scope);

        let source = v8::String::new(handle_scope, source)?;
        let strip_enum_and_namespace = v8::Boolean::new(handle_scope, strip_enum_and_namespace);

        let result = process_ts_func
            .call(handle_scope, global.cast(), &[source.cast(), strip_enum_and_namespace.cast()])?;
        serde_v8::from_v8::<Option<TranspileOutput>>(handle_scope, result).ok()?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ensure_v8_init() {
        static INIT_ONCE: Once = Once::new();
        INIT_ONCE.call_once(|| {
            let platform = v8::new_default_platform(0, false).make_shared();
            v8::V8::initialize_platform(platform);
            v8::V8::initialize();
        });
    }

    #[test]
    fn invalid_syntax() {
        ensure_v8_init();

        let mut tsc = Tsc::new();

        assert_eq!(tsc.process_ts("let a: string =", true), None);
    }

    #[test]
    fn script_kind() {
        ensure_v8_init();

        let mut tsc = Tsc::new();
        assert_eq!(
            tsc.process_ts("let a: string = 1", true).unwrap().kind,
            SourceKind::Script
        );
    }
    #[test]
    fn module_kind() {
        ensure_v8_init();

        let mut tsc = Tsc::new();
        assert_eq!(
            tsc.process_ts("export let a: string = 1", true).unwrap().kind,
            SourceKind::Module
        );
    }
}
