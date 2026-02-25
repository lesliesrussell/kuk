mod commands;

pub use commands::BoardCmd;
pub use commands::Cli;
pub use commands::Commands;

use crate::error::Result;
use crate::storage::Store;

pub fn run(cli: Cli) -> Result<()> {
    let repo = cli.repo.unwrap_or_else(|| std::env::current_dir().unwrap());
    let store = Store::new(&repo);
    let json_output = cli.json;

    match cli.command {
        Some(Commands::Init { board_name }) => commands::init(&store, &board_name),
        Some(Commands::List { board }) => commands::list(&store, board.as_deref(), json_output),
        Some(Commands::Add {
            title,
            to,
            label,
            assignee,
        }) => commands::add(&store, &title, &to, label, assignee, json_output),
        Some(Commands::Move { id, to }) => commands::move_card(&store, &id, &to, json_output),
        Some(Commands::Hoist { id }) => commands::hoist(&store, &id, json_output),
        Some(Commands::Demote { id }) => commands::demote(&store, &id, json_output),
        Some(Commands::Archive { id }) => commands::archive(&store, &id, json_output),
        Some(Commands::Delete { id }) => commands::delete(&store, &id, json_output),
        Some(Commands::Label { id, action, tag }) => {
            commands::label(&store, &id, &action, &tag, json_output)
        }
        Some(Commands::Assign { id, user }) => commands::assign(&store, &id, &user, json_output),
        Some(Commands::Board { command }) => commands::board(&store, command, json_output),
        Some(Commands::Projects) => commands::projects(json_output),
        Some(Commands::Tui) => crate::tui::run_tui(&repo),
        Some(Commands::Serve { port, mcp }) => {
            let rt = tokio::runtime::Runtime::new()
                .map_err(|e| crate::error::KukError::Other(format!("Runtime error: {e}")))?;
            rt.block_on(crate::server::serve(repo, port, mcp))
        }
        Some(Commands::Mcp) => crate::mcp_stdio::run(&store),
        Some(Commands::Doctor) => commands::doctor(&store),
        Some(Commands::Version) => commands::version(),
        None => commands::default_action(),
    }
}
