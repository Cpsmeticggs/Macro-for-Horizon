fn ui_render(game: &crate::game::Game<'_>, batch: &mut horizon_client::hud::HudBatch, screen: &crate::game::native_hooks::NativeScreenView<'_>) {
    crate::game::macro_ui::render(game, batch, screen);
}
