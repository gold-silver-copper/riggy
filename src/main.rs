use riggy::{
    app::service::GameService,
    llm::{AnyBackend, LlmBackend},
    logging, tui,
};
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _logging = logging::init()?;
    let backend = AnyBackend::from_env()?;
    info!(backend = %backend.label(), "starting riggy");
    let game = GameService::new(backend)?;
    tui::run(game).await
}
