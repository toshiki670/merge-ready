#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PromptSignal {
    Conflict,
    UpdateBranch,
    SyncUnknown,
    CiFail,
    CiAction,
    ReviewRequested,
    MergeReady,
}
