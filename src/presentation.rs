/// ドメイン結果を `stdout` に出力する（末尾改行なし）
pub fn display(tokens: &[&str]) {
    print!("{}", tokens.join(" "));
}

/// 単一トークンを `stdout` に出力する（末尾改行なし）
pub fn display_error(token: &str) {
    print!("{token}");
}
