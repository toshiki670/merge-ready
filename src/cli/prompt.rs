use crate::cli::args::PromptArgs;

pub(crate) fn run(args: &PromptArgs) {
    if args.refresh {
        crate::application::prompt::run_refresh();
    } else if args.no_cache {
        crate::application::prompt::run_direct();
    } else {
        crate::application::prompt::run_cached();
    }
}
