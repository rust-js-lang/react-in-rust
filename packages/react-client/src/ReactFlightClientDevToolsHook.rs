use std::marker::PhantomData;

use js::{JsError, JsObject, Unknown};
use shared::environment::__DEV__;
use webapi::console;

/// React DevTools' global hook, as the Flight client reads it: what
/// DevTools, or Fast Refresh's runtime, installs before React loads.
pub struct DevToolsHook(PhantomData<JsObject>);

impl DevToolsHook {
    #[cfg_attr(rust_js, rust_js::link_name = "inject")]
    pub fn inject(
        &self,
        internals: &Unknown,
    ) -> Result<Option<&'static Unknown>, &'static JsError> {
        let _ = internals;
        unreachable!()
    }
}

unsafe extern "Rust" {
    #[link_name = "__REACT_DEVTOOLS_GLOBAL_HOOK__"]
    safe static REACT_DEVTOOLS_GLOBAL_HOOK: &'static DevToolsHook;
}

pub fn inject_internals(internals: &'static Unknown) -> bool {
    if !js::is_defined(REACT_DEVTOOLS_GLOBAL_HOOK) {
        // No DevTools
        return false;
    }
    let hook = REACT_DEVTOOLS_GLOBAL_HOOK;
    if js::truthy(js::get(hook, "isDisabled")) {
        // This isn't a real property on the hook, but it can be set to opt out
        // of DevTools integration and associated warnings and logs.
        // https://github.com/react/react/issues/3877
        return true;
    }
    if !js::truthy(js::get(hook, "supportsFlight")) {
        // DevTools exists, even though it doesn't support Flight.
        return true;
    }
    if let Err(err) = hook.inject(internals) {
        // Catch all errors because it is unsafe to throw during initialization.
        if __DEV__ {
            console::error((
                "React instrumentation encountered an error: %o.",
                err.as_unknown(),
            ));
        }
    }
    if js::truthy(js::get(hook, "checkDCE")) {
        // This is the real DevTools.
        return true;
    }
    // This is likely a hook installed by Fast Refresh runtime.
    false
}
