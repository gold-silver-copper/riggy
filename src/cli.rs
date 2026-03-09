use std::io::{self, Write};
use std::path::PathBuf;

use anyhow::Result;

use crate::llm::LlmBackend;
use crate::simulation::Game;

pub async fn run<B: LlmBackend>(game: &mut Game<B>) -> Result<()> {
    println!(
        "Riggy starting with backend `{}`.\nType `help` for commands.\n",
        game.backend_name()
    );
    println!("{}", game.render_location_summary());

    let stdin = io::stdin();
    loop {
        print!("{}", prompt(game));
        io::stdout().flush()?;

        let mut input = String::new();
        stdin.read_line(&mut input)?;
        if input.is_empty() {
            break;
        }

        let output = game.handle_input(input.trim_end()).await?;
        if !output.text.is_empty() {
            println!("{}", output.text);
        }
        if output.should_quit {
            break;
        }
    }

    Ok(())
}

fn prompt<B: LlmBackend>(game: &Game<B>) -> String {
    match game.dialogue_partner_name() {
        Some(name) => format!("talk:{}> ", name),
        None => "> ".to_string(),
    }
}

pub fn default_save_path() -> PathBuf {
    PathBuf::from("savegame.json")
}
