fn ui_key(game: &crate::game::Game<'_>, key: &str, canceled: &mut bool) {
    crate::game::macro_ui::key(game, key, canceled);
}
