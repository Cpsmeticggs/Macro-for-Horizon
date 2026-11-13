fn ui_char(game: &crate::game::Game<'_>, c: char, canceled: &mut bool) {
    crate::game::macro_ui::character(game, c, canceled);
}
