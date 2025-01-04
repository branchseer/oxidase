use std::{cell::RefCell, sync::Once};

use v8::OwnedIsolate;

pub fn with_isolate<R, F: FnOnce(&mut OwnedIsolate) -> R>(f: F) -> R {
    static INIT_ONCE: Once = Once::new();
    INIT_ONCE.call_once(|| {
        let platform = v8::new_default_platform(0, false).make_shared();
        v8::V8::initialize_platform(platform);
        v8::V8::initialize();
    });

    thread_local! { static ISOLATE: RefCell<OwnedIsolate> = RefCell::new(v8::Isolate::new(v8::CreateParams::default())) };
    ISOLATE.with_borrow_mut(f)
}
