
use v8::Local;

#[derive(Debug, thiserror::Error)]
pub enum EvalError {
    #[error("failed to evaluate the module source: {message} (line {line}, column {column})")]
    ModuleEvaluationError {
        message: String,
        line: usize,
        column: usize,
    },
    #[error("exported items aren't valid jsons: {0:?}")]
    InvalidJson(serde_v8::Error),
}

impl EvalError {
    fn from_evaluation_exception(
        scope: &mut v8::HandleScope,
        exception: v8::Local<'_, v8::Value>,
    ) -> Self {
        let exception_message = v8::Exception::create_message(scope, exception);
        EvalError::ModuleEvaluationError {
            message: exception_message.get(scope).to_rust_string_lossy(scope),
            line: exception_message.get_line_number(scope).unwrap_or(0),
            column: exception_message.get_start_column(),
        }
    }
}

pub fn eval(src: &str) -> Result<serde_json::Value, EvalError> {
    oxidase_tsc::v8_utils::with_isolate(|isolate| {
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
            module.instantiate_module(scope, |_, _, _, _| None)?;
            module.evaluate(scope)?;
            Some(module)
        }()
        .ok_or_else(|| {
            if let Some(exception) = scope.exception() {
                EvalError::from_evaluation_exception(scope, exception)
            } else {
                EvalError::ModuleEvaluationError {
                    message: "Unknown error".to_owned(),
                    line: 0,
                    column: 0,
                }
            }
        })?;

        if evaluated_module.get_status() != v8::ModuleStatus::Evaluated {
            let exception = evaluated_module.get_exception();
            return Err(EvalError::from_evaluation_exception(scope, exception));
        }

        let module_namespace = evaluated_module.get_module_namespace();

        // TODO: check invalid values such as functions. They're currently converted to empty objects.
        let json = serde_v8::from_v8::<serde_json::Value>(scope, module_namespace)
            .map_err(EvalError::InvalidJson)?;
        Ok(json)
    })
}

#[cfg(test)]
mod tests {
    

    use super::*;

    #[test]
    fn basic() {
        let json = eval("export const a = {  b: 2 }; export const x = [null, ''];").unwrap();
        assert_eq!(
            json,
            serde_json::json!({ "a": { "b": 2}, "x": [null, ""]  })
        )
    }

    #[test]
    fn syntax_error() {
        assert!(matches!(
            eval("!"),
            Err(EvalError::ModuleEvaluationError { .. })
        ));
    }
    #[test]
    fn runtime_error() {
        assert!(matches!(
            eval("throw new Error()"),
            Err(EvalError::ModuleEvaluationError { .. })
        ));
    }
}
