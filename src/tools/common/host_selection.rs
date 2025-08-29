use once_cell::sync::OnceCell;
use std::sync::Mutex;

static SELECTED_FE_HOST: OnceCell<Mutex<Option<String>>> = OnceCell::new();
static SELECTED_BE_HOST: OnceCell<Mutex<Option<String>>> = OnceCell::new();

fn storage(cell: &OnceCell<Mutex<Option<String>>>) -> &Mutex<Option<String>> {
    cell.get_or_init(|| Mutex::new(None))
}

pub fn set_selected_host(is_be: bool, host: String) {
    let cell = if is_be {
        &SELECTED_BE_HOST
    } else {
        &SELECTED_FE_HOST
    };
    if let Ok(mut guard) = storage(cell).lock() {
        *guard = Some(host);
    }
}

pub fn get_selected_host(is_be: bool) -> Option<String> {
    let cell = if is_be {
        &SELECTED_BE_HOST
    } else {
        &SELECTED_FE_HOST
    };
    storage(cell).lock().ok().and_then(|g| g.clone())
}
