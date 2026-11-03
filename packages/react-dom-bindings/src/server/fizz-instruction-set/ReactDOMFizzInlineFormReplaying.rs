use crate::react_dom_fizz_instruction_set_shared::listen_to_form_submissions_for_replaying;

// TODO: Export a helper function that throws the error from javascript URLs instead.
// We can do that here since we mess with globals anyway and we can guarantee it has loaded.
// It makes less sense in the external runtime since it's async loaded and doesn't expose globals
// so we might have to have two different URLs.

js::on_load! {
    listen_to_form_submissions_for_replaying();
}
