mod commands;

pub use commands::Cli;
pub use commands::Commands;

use crate::error::Result;

pub fn run(cli: Cli) -> Result<()> {
    let repo = cli.repo.unwrap_or_else(|| std::env::current_dir().unwrap());
    let json_output = cli.json;

    match cli.command {
        Some(Commands::Init) => commands::init(&repo),
        Some(Commands::Projects) => commands::projects(json_output),
        Some(Commands::Sync { dry_run }) => commands::sync(&repo, dry_run, json_output),
        Some(Commands::Link { card_id, url }) => commands::link(&repo, &card_id, &url, json_output),
        Some(Commands::Branch { card_id }) => commands::branch(&repo, &card_id, json_output),
        Some(Commands::Pr { card_id }) => commands::pr(&repo, &card_id, json_output),
        Some(Commands::Velocity { weeks, target }) => {
            commands::velocity(&repo, weeks, target.as_deref(), json_output)
        }
        Some(Commands::Burndown { sprint }) => {
            commands::burndown(&repo, sprint.as_deref(), json_output)
        }
        Some(Commands::Roadmap { weeks }) => commands::roadmap(&repo, weeks, json_output),
        Some(Commands::ReleaseNotes { since }) => {
            commands::release_notes(&repo, since.as_deref(), json_output)
        }
        Some(Commands::Sprint { command }) => commands::sprint(&repo, command, json_output),
        Some(Commands::Stats) => commands::stats(&repo, json_output),
        Some(Commands::Mcp) => {
            let store = kuk::storage::Store::new(&repo);
            crate::mcp_stdio::run(&store, &repo)
        }
        Some(Commands::Doctor) => commands::doctor(&repo),
        Some(Commands::Version) => commands::version(),
        None => commands::default_action(),
    }
}
