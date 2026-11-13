fn ui_mouse(game: &crate::game::Game<'_>, screen: &crate::game::native_hooks::NativeScreenView<'_>, button: i32, canceled: &mut bool) {
    crate::game::macro_ui::mouse(game, screen, button, canceled);
}
