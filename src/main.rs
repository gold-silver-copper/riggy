use riggy::{cli, llm::AnyBackend, simulation::Game};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let backend = AnyBackend::from_env()?;
    let mut game = Game::new(backend);
    cli::run(&mut game).await
}
