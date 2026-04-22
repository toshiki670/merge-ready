/// 個別チェックのバケット種別
///
/// `Fail`/`Cancel`/`ActionRequired` は `ci-fail`/`ci-action` 判定に使用する。
pub enum CheckBucket {
    Fail,
    Cancel,
    ActionRequired,
    Other,
}
