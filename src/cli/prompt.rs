mod args;
mod cached;
mod direct;
mod refresh;

pub(super) use args::{PROMPT_AFTER_HELP, PromptArgs};

pub(crate) fn run(args: &PromptArgs) {
    if args.refresh {
        match args.repo_id.as_deref() {
            Some(id) => {
                // 親プロセスから --repo-id で渡された場合（通常パス）: git 再取得なし
                refresh::run_refresh(id);
            }
            None => {
                // 手動実行など親なしの場合: git から取得（この場合ロック孤児は発生しない）
                if let Some(id) = crate::infra::repo_id::get() {
                    refresh::run_refresh(&id);
                }
            }
        }
    } else if args.no_cache {
        direct::run_direct();
    } else {
        cached::run_cached();
    }
}
