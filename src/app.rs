use std::process::ExitCode;

use clap::CommandFactory;

use crate::adapters::ConfigAdapter;
use crate::cli::{Cli, Command};
use crate::contexts::daemon::application::cache as daemon_cache_app;
use crate::contexts::daemon::infrastructure::daemon_client::DaemonClient;
use crate::contexts::evaluation::domain::pr_state::PrStateRepository;
use crate::contexts::evaluation::infrastructure::{gh::GhClient, logger::Logger};
use crate::contexts::evaluation::interface::presentation::{PresentationConfigPort, Presenter};

#[allow(clippy::needless_pass_by_value)]
pub fn run(cli: Cli) -> ExitCode {
    match cli.command {
        Some(Command::Config) => {
            let config_path = crate::contexts::config::infrastructure::toml_loader::config_path();
            crate::contexts::config::interface::cli::run(config_path.as_deref())
        }
        Some(Command::Daemon(args)) => {
            crate::contexts::evaluation::infrastructure::logger::init();
            let lifecycle =
                crate::contexts::daemon::infrastructure::daemon_lifecycle::DaemonLifecycle::new(
                    |repo_id: &str, cwd: &std::path::Path| {
                        let repo_id = repo_id.to_owned();
                        let client = GhClient::new_in(cwd.to_path_buf());
                        let (tokens, error) =
                            crate::contexts::evaluation::application::prompt::fetch_output(
                                &client, &Logger,
                            );
                        let config = ConfigAdapter::load();
                        let output = if let Some(err) = error {
                            config.render_error_token(err)
                        } else {
                            Presenter::new(config).render_to_string(&tokens)
                        };
                        // エラー時は is_terminal = false（再試行が必要なため）
                        // OnceLock キャッシュ済みの fetch_lifecycle を呼び出すため追加 API 呼び出しなし
                        let is_terminal = error.is_none()
                            && tokens.is_empty()
                            && client.fetch_lifecycle().is_ok_and(|l| l.is_terminal());
                        daemon_cache_app::update(&DaemonClient, &repo_id, &output, is_terminal);
                    },
                );
            crate::contexts::daemon::interface::cli::daemon::run(args.subcommand, &lifecycle)
        }
        None => {
            let _ = Cli::command().print_help();
            ExitCode::SUCCESS
        }
    }
}
