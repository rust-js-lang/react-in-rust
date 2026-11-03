use crate::react_dom_fizz_instruction_set_shared::{complete_segment, set_rs};

js::on_load! {
    // This is a string so Closure's advanced compilation mode doesn't mangle it.
    set_rs(&complete_segment);
}
