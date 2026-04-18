use super::super::domain::config::CURRENT_VERSION;
use super::super::domain::repository::ConfigRepository;

pub fn run(repo: &impl ConfigRepository) {
    let mut config = repo.load();
    if config.version == CURRENT_VERSION {
        return;
    }
    config.version = CURRENT_VERSION;
    config.fill_defaults();
    repo.save(&config);
}

pub fn default_config_toml() -> String {
    format!(
        "\
version = {CURRENT_VERSION}

# merge-ready configuration
# All fields are optional; omit a section to use built-in defaults.

# [merge_ready]
# symbol = \"✓\"
# label = \"merge-ready\"
# format = \"$symbol $label\"

# [conflict]
# symbol = \"✗\"
# label = \"conflict\"

# [update_branch]
# symbol = \"✗\"
# label = \"update-branch\"

# [sync_unknown]
# symbol = \"?\"
# label = \"sync-unknown\"

# [ci_fail]
# symbol = \"✗\"
# label = \"ci-fail\"

# [ci_action]
# symbol = \"⚠\"
# label = \"ci-action\"

# [review]
# symbol = \"⚠\"
# label = \"review\"
"
    )
}
