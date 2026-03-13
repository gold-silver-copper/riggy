use riggy::{app::service::GameService, llm::AnyBackend, tui};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let backend = AnyBackend::from_env()?;
    let game = GameService::new(backend)?;
    tui::run(game).await
}
