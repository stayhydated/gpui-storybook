use es_fluent_manager_embedded as i18n_manager;

es_fluent_manager_embedded::define_i18n_module!();

pub fn init() {
    i18n_manager::init();
}

pub use i18n_manager::select_language as change_locale;
