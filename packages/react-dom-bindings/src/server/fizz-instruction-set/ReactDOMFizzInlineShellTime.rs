use webapi::performance;
use webapi::window_global::request_animation_frame;

use crate::react_dom_fizz_instruction_set_shared::set_rt;

js::on_load! {
    // Track the paint time of the shell
    request_animation_frame(Box::new(|_| {
        set_rt(performance.now());
    }));
}
