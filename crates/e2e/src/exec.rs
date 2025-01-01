use std::{cell::RefCell, ops::Deref};

use v8::{Local, ScriptOrigin};

#[derive(Debug)]
pub enum EvalError {
    ModuleEvaluationError {
        message: String,
        line: usize,
        column: usize,
    },
    InvalidJson(serde_v8::Error)
}

impl EvalError {
    fn from_evaluation_exception(scope: &mut v8::HandleScope, exception: v8::Local<'_, v8::Value>) -> Self {
        let exception_message = v8::Exception::create_message(scope, exception);
        EvalError::ModuleEvaluationError {
            message: exception_message.get(scope).to_rust_string_lossy(scope),
            line: exception_message.get_line_number(scope).unwrap(),
            column: exception_message.get_start_column(),
        }
    }
}

pub fn eval(src: &str) -> Result<serde_json::Value, EvalError> {
    thread_local! { static ISOLATE: RefCell<Option<v8::OwnedIsolate>> = RefCell::new(None) }
    ISOLATE.with_borrow_mut(|isolate| {
        let isolate = isolate.get_or_insert_with(|| v8::Isolate::new(v8::CreateParams::default()));

        let handle_scope = &mut v8::HandleScope::new(isolate);
        let context = v8::Context::new(handle_scope, Default::default());

        let scope = &mut v8::ContextScope::new(handle_scope, context);
        let code = v8::String::new(scope, src).unwrap();
        // let script = v8::Script::compile(scope, code, None).unwrap();

        let res_name = v8::String::new(scope, "a.js").unwrap().into();
        let origin = v8::ScriptOrigin::new(
            scope, res_name, 0, 0, false, 0, None, false, false, true, None,
        );
        let mut source = v8::script_compiler::Source::new(code, Some(&origin));

        let scope = &mut v8::TryCatch::new(scope);

        let evaluated_module = || -> Option<Local<'_, v8::Module>> {
            let module = v8::script_compiler::compile_module(scope, &mut source)?;
            module.instantiate_module(scope, module_resolve_callback)?;
            module.evaluate(scope)?;
            Some(module)
        }()
        .ok_or_else(|| {
            let exception = scope.exception().unwrap();
            EvalError::from_evaluation_exception(scope, exception)
        })?;

        if evaluated_module.get_status() != v8::ModuleStatus::Evaluated {
            let exception = evaluated_module.get_exception();
            return Err(EvalError::from_evaluation_exception(scope, exception))
        }

        let module_namespace = evaluated_module.get_module_namespace();
        dbg!(v8::json::stringify(scope, module_namespace));
        let json = serde_v8::from_v8::<serde_json::Value>(scope, module_namespace).map_err(|err| EvalError::InvalidJson(err))?;
        Ok(json)
    })
}

fn module_resolve_callback<'a>(
    _context: v8::Local<'a, v8::Context>,
    _specifier: v8::Local<'a, v8::String>,
    _import_assertions: v8::Local<'a, v8::FixedArray>,
    _referrer: v8::Local<'a, v8::Module>,
) -> Option<v8::Local<'a, v8::Module>> {
    None
}

#[cfg(test)]
mod tests {
    use std::sync::Once;

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
    fn basic() {
        ensure_v8_init();
        dbg!(eval("export const z = {  b: 2, c() {} }"));
    }
}
