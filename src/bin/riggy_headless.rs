use riggy::{headless, logging};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _logging = logging::init()?;
    headless::run_cli(std::env::args().skip(1)).await
}
