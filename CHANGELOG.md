# Changelog

All notable changes to this project will be documented in this file.

## [1.0.4] - 2026-06-14
### Fixes
- 空・相対の XDG_CONFIG_HOME/XDG_CACHE_HOME を無効として扱う ([#418](https://github.com/toshiki670/merge-ready/pull/418)) ([`981416a`](https://github.com/toshiki670/merge-ready/commit/981416a1ec7448dd5ec040a939bb5239f709ca7d))
- Logger のレベルを MERGE_READY_LOG_LEVEL で可変にし既定を warn へ ([#419](https://github.com/toshiki670/merge-ready/pull/419)) ([`312956c`](https://github.com/toshiki670/merge-ready/commit/312956c5b77564faa1571d1c3da95cf35b18e348))
- 相対 gitdir（submodule 等）を .git 基準で解決し branch を保持する ([#422](https://github.com/toshiki670/merge-ready/pull/422)) ([`3ae269f`](https://github.com/toshiki670/merge-ready/commit/3ae269f807ce93c2cf16f47448834e90a5d30939))
- 認証失敗ログに gh stderr の原因を残す ([#426](https://github.com/toshiki670/merge-ready/pull/426)) ([`091ffc6`](https://github.com/toshiki670/merge-ready/commit/091ffc67854669a2fe05971e93b91082dfda7016))
- Error.log のタイムスタンプを日付込み RFC3339 にする ([#427](https://github.com/toshiki670/merge-ready/pull/427)) ([`54997d3`](https://github.com/toshiki670/merge-ready/commit/54997d3776f26525684a43911a3c3fb2119cb051))
- Backoff の reset 時刻をボトルネック側リソース基準にする ([#428](https://github.com/toshiki670/merge-ready/pull/428)) ([`63aabe7`](https://github.com/toshiki670/merge-ready/commit/63aabe74e59b7af0bc48920dcfe94219b378c458))
- Watch テーブルの列揃えを端末表示幅（Unicode Annex #11）基準にする ([#429](https://github.com/toshiki670/merge-ready/pull/429)) ([`330ac5f`](https://github.com/toshiki670/merge-ready/commit/330ac5f385a410808f3b92e127904caa03d9128e))
- Unix socket と base_dir の権限を制限する ([#433](https://github.com/toshiki670/merge-ready/pull/433)) ([`8eb1d96`](https://github.com/toshiki670/merge-ready/commit/8eb1d96cc39ab31994fa70e4f32198c680978888))
- Daemon 応答を最後まで読み取る ([#434](https://github.com/toshiki670/merge-ready/pull/434)) ([`47c575d`](https://github.com/toshiki670/merge-ready/commit/47c575d69576734a3bcff53838f9e3ec11363168))
- Error.log をサイズベースでローテーションする ([#436](https://github.com/toshiki670/merge-ready/pull/436)) ([`bf86f19`](https://github.com/toshiki670/merge-ready/commit/bf86f1929622b7008fab69d3aef91d0086eadac4))
- Hot path ベンチを安定して実行できるようにする ([#438](https://github.com/toshiki670/merge-ready/pull/438)) ([`ab62df4`](https://github.com/toshiki670/merge-ready/commit/ab62df4a2500a38bdb078a076aa3debd4773af3d))
- Daemon 応答読み取り全体に総タイムアウトを設ける ([#437](https://github.com/toshiki670/merge-ready/pull/437)) ([`596507b`](https://github.com/toshiki670/merge-ready/commit/596507bc61796d06a3d450eac75711f21f051c8d))
### Performance
- PID 生存確認・SIGTERM 送信を rustix の safe API に置換する ([#420](https://github.com/toshiki670/merge-ready/pull/420)) ([`4d72ea3`](https://github.com/toshiki670/merge-ready/commit/4d72ea3512ed7d5cd80c19a87926ec6ed99c0355))
- Calculating / CONFLICTING 時に compare API をスキップする ([#421](https://github.com/toshiki670/merge-ready/pull/421)) ([`3907746`](https://github.com/toshiki670/merge-ready/commit/3907746e635d45cfcfac467912666d927576f081))


## [1.0.3] - 2026-05-31
### Fixes
- Uptime/CACHED AT の表示を humantime で人間に読みやすい形式に変更 ([#395](https://github.com/toshiki670/merge-ready/pull/395)) ([`a2bee26`](https://github.com/toshiki670/merge-ready/commit/a2bee2699fbb5fd9ec17e7c6792cc75348184b58))
### Performance
- 設定をハッシュキャッシュ + 非同期ロードし render を前計算する ([#397](https://github.com/toshiki670/merge-ready/pull/397)) ([`25003bb`](https://github.com/toshiki670/merge-ready/commit/25003bbb5bf87c6b47c6a9a25f94ba2b62e74c2b))


## [1.0.2] - 2026-05-31
### Bug Fixes
- Substitute_vars が非 ASCII を含む format を文字化けさせる問題を修正 ([#388](https://github.com/toshiki670/merge-ready/pull/388)) ([`e5b481b`](https://github.com/toshiki670/merge-ready/commit/e5b481b91fdc790e2e4e8a3998b92da872a79075))
- クロスバージョンの旧 daemon を旧命名ディレクトリ含め停止する ([#390](https://github.com/toshiki670/merge-ready/pull/390)) ([#391](https://github.com/toshiki670/merge-ready/pull/391)) ([`49b0ff5`](https://github.com/toshiki670/merge-ready/commit/49b0ff5977c4fee68fdd4dcc74b44db257f008a3))
### Performance
- Cache transition で entries の二重 deep clone を解消 ([#387](https://github.com/toshiki670/merge-ready/pull/387)) ([`a1ffd6d`](https://github.com/toshiki670/merge-ready/commit/a1ffd6d58b7199343b5384d84465c11fc71f7f74))
- Process_update で output/pr_outputs の不要コピーを削減 ([#386](https://github.com/toshiki670/merge-ready/pull/386)) ([#393](https://github.com/toshiki670/merge-ready/pull/393)) ([`8dbe19d`](https://github.com/toshiki670/merge-ready/commit/8dbe19de6f3e2acdbb7258524921341dc1e22fc5))


## [1.0.1] - 2026-05-29
### Bug Fixes
- Compare API パスのブランチ名を URL エンコードする ([#378](https://github.com/toshiki670/merge-ready/pull/378)) ([`5bec73d`](https://github.com/toshiki670/merge-ready/commit/5bec73d9664c954af08f8e599e871d6667a2d797))
- 非 Linux でも temp ディレクトリを uid で名前空間分離する ([#380](https://github.com/toshiki670/merge-ready/pull/380)) ([`b2ae8ca`](https://github.com/toshiki670/merge-ready/commit/b2ae8cab07399622d478d69f0825d56f034ca349))
- Daemon ソケットの入力を検証し、出力の制御文字を除去する ([#381](https://github.com/toshiki670/merge-ready/pull/381)) ([`6962bd4`](https://github.com/toshiki670/merge-ready/commit/6962bd496489d1fd57aed3a78bf8c2501af8e74c))
- Gh spawn 失敗時の panic を回避しエラーとして扱う ([#379](https://github.com/toshiki670/merge-ready/pull/379)) ([`c9f5717`](https://github.com/toshiki670/merge-ready/commit/c9f57172f043b45166a490dc8688a1f2eb481cc2))


## [1.0.0] - 2026-05-23
### Performance
- PR 不在パスでの git branch --show-current 重複呼び出しを解消する ([#366](https://github.com/toshiki670/merge-ready/pull/366)) ([`7a118f2`](https://github.com/toshiki670/merge-ready/commit/7a118f2b9ae95e53961c6f53bd4867b26a1cb6fe))
- レート制限コストモデルを実 API コール数に整合させる ([#367](https://github.com/toshiki670/merge-ready/pull/367)) ([`a72ec97`](https://github.com/toshiki670/merge-ready/commit/a72ec970e585ffd3d99709ac8a0ede3c5bca14f9))
- Refresh を単一 GraphQL クエリへ集約し gh subprocess を 2N+1 → N+1 にする ([#368](https://github.com/toshiki670/merge-ready/pull/368)) ([`03dabb3`](https://github.com/toshiki670/merge-ready/commit/03dabb323404054b2113962ed7590bc15d9b8a21))


## [0.8.1] - 2026-05-21
### Performance
- Refresh 時の PR ごとの `gh repo view --json nameWithOwner` を撤廃する ([#363](https://github.com/toshiki670/merge-ready/pull/363)) ([`8b9551e`](https://github.com/toshiki670/merge-ready/commit/8b9551e8ede041bfaca0ac21361e38856364dc8d))


## [0.8.0] - 2026-05-20
### Features
- [**BREAKING**] Aggregate identical statuses and list PR numbers in prompt ([#356](https://github.com/toshiki670/merge-ready/pull/356)) ([`d3baef5`](https://github.com/toshiki670/merge-ready/commit/d3baef50b2a05b60cfa2d800b32e98135ccc3118))


## [0.7.5] - 2026-05-19
### Bug Fixes
- Right-align CACHED AT column ([#335](https://github.com/toshiki670/merge-ready/pull/335)) ([`6ed733e`](https://github.com/toshiki670/merge-ready/commit/6ed733ec4314cc2689c824efe116029d2769e2a4))


## [0.7.4] - 2026-05-17
### Features
- Scale refresh interval based on gh rate limit ([#274](https://github.com/toshiki670/merge-ready/pull/274)) ([#328](https://github.com/toshiki670/merge-ready/pull/328)) ([`3af5c0e`](https://github.com/toshiki670/merge-ready/commit/3af5c0efea01dbff260faf545bdb0949bfc01ac9))
- Self-terminate inner daemon when own socket disappears ([#332](https://github.com/toshiki670/merge-ready/pull/332)) ([`a858803`](https://github.com/toshiki670/merge-ready/commit/a858803825f39c1b58d2f276f9fc0264b00deb45))


## [0.7.3] - 2026-05-07
### Bug Fixes
- Daemon の終了処理を安定化して coverage 実行のハングを防ぐ ([#281](https://github.com/toshiki670/merge-ready/pull/281)) ([`4ac2309`](https://github.com/toshiki670/merge-ready/commit/4ac230922b56146d6689448efd25a05309345ebf))
### Features
- Watch のエントリ一覧を CWD → BRANCH → PR 昇順でソートする ([#275](https://github.com/toshiki670/merge-ready/pull/275)) ([`25acfc4`](https://github.com/toshiki670/merge-ready/commit/25acfc48971383dff182531f70f9cf294026f67d))


## [0.7.2] - 2026-05-06
### Bug Fixes
- Daemon 起動レースを防ぐ ([#266](https://github.com/toshiki670/merge-ready/pull/266)) ([`c385de5`](https://github.com/toshiki670/merge-ready/commit/c385de574bda1d3b255b76821855f994d721429e))
### Features
- #256 1ブランチに複数PRがある場合に全PRの状態を並べて表示する ([#262](https://github.com/toshiki670/merge-ready/pull/262)) ([`ccdf9bb`](https://github.com/toshiki670/merge-ready/commit/ccdf9bb84e2741c2a183b8a3549a10fd43946921))
- #260 conditional format strings のサポート ([#263](https://github.com/toshiki670/merge-ready/pull/263)) ([`ccf8764`](https://github.com/toshiki670/merge-ready/commit/ccf8764234e2c0be62dfb2ded1ca207c220f481c))
- Watch のエントリ一覧に PR 番号を表示する ([#268](https://github.com/toshiki670/merge-ready/pull/268)) ([`0d91165`](https://github.com/toshiki670/merge-ready/commit/0d9116514c85073b4b79dda5c0e6835cd9697a0d))


## [0.7.1] - 2026-05-05
### Bug Fixes
- Daemon が複数起動する競合と中間プロセス問題を修正する ([#251](https://github.com/toshiki670/merge-ready/pull/251)) ([`bd6a36f`](https://github.com/toshiki670/merge-ready/commit/bd6a36fd0cede80fb9b4603963cf8b8bcc8fbc2e))
### Features
- Add completions subcommand via clap_complete ([#254](https://github.com/toshiki670/merge-ready/pull/254)) ([`86da9d8`](https://github.com/toshiki670/merge-ready/commit/86da9d8d318b65e259a46af51a44613a93fbd609))


## [0.7.0] - 2026-05-04
### Bug Fixes
- Daemon_server::run の Result<(), ()> を DaemonError に置き換える ([#235](https://github.com/toshiki670/merge-ready/pull/235)) ([`127adc5`](https://github.com/toshiki670/merge-ready/commit/127adc58de063b8bdccd5208cf385030908c0611))
- Gh に対応していないリポジトリではメッセージを表示しない ([#236](https://github.com/toshiki670/merge-ready/pull/236)) ([`0a507e3`](https://github.com/toshiki670/merge-ready/commit/0a507e3162844b0d717750596bf81cb5cd0f029d))
### Features
- キャッシュエントリの最新ステータスを常時表示する watch コマンドを追加する ([#240](https://github.com/toshiki670/merge-ready/pull/240)) ([`3b27be8`](https://github.com/toshiki670/merge-ready/commit/3b27be8875ba666b33f6e9082269d5864404cf78))
### Refactor
- [**BREAKING**] CLI 型をバイナリ側に移動し contexts から clap 依存を除去する ([#246](https://github.com/toshiki670/merge-ready/pull/246)) ([`6481b55`](https://github.com/toshiki670/merge-ready/commit/6481b5500aa00c47bfe333c6f05c08e26a0a36e3))


## [0.6.0] - 2026-05-02
### Bug Fixes
- Remove needless pass-by-value allow in cli::run ([#226](https://github.com/toshiki670/merge-ready/pull/226)) ([`2ed55c9`](https://github.com/toshiki670/merge-ready/commit/2ed55c910d6f9a7deb2ed5faf271c07c65305f55))
- Align error category logging for auth and timeout ([#227](https://github.com/toshiki670/merge-ready/pull/227)) ([`7692847`](https://github.com/toshiki670/merge-ready/commit/76928475a5aa698717f08d2d28e9f2ff33743f39))
### Features
- [**BREAKING**] Split daemon_command into daemon_start/stop/status_command ([#230](https://github.com/toshiki670/merge-ready/pull/230)) ([`396cbea`](https://github.com/toshiki670/merge-ready/commit/396cbeabc3d85d4f85e330c449bd14d5faab0a8b))
- Format フィールドで Starship 風の色・スタイル指定をサポートする ([#231](https://github.com/toshiki670/merge-ready/pull/231)) ([`4b81a93`](https://github.com/toshiki670/merge-ready/commit/4b81a937bd09cef2734db2626bb1e2c5bf3fb773))


## [0.5.3] - 2026-04-30
### Bug Fixes
- Docs.rs ビルドのために [lib] ターゲットを追加し app.rs を lib.rs に統合する ([#219](https://github.com/toshiki670/merge-ready/pull/219)) ([`541b318`](https://github.com/toshiki670/merge-ready/commit/541b318cb0842c09c3a1814de1085c8978871e3f))


## [0.5.2] - 2026-04-29
### Features
- Daemon の定期取得サイクルをリポジトリごとに適応的に制御する ([#198](https://github.com/toshiki670/merge-ready/pull/198)) ([`5e25824`](https://github.com/toshiki670/merge-ready/commit/5e258242a304c2105f35a621a30c73e65aa6b6c0))
### Performance
- リリースバイナリにデバッグシンボルの strip を追加する ([#199](https://github.com/toshiki670/merge-ready/pull/199)) ([`cf9b930`](https://github.com/toshiki670/merge-ready/commit/cf9b930b47480851f2d970ff55dfe3e0020c2dd1))


## [0.5.1] - 2026-04-28
### Features
- BLOCKED かつ理由不明のとき ? Check merge blocker を表示する ([#192](https://github.com/toshiki670/merge-ready/pull/192)) ([`d63f075`](https://github.com/toshiki670/merge-ready/commit/d63f075d11c9e92fe72373eb9391a8ae260244b2))


## [0.5.0] - 2026-04-27
### Features
- ReviewDecision == "REVIEW_REQUIRED" のとき @ assign-reviewer を表示する ([#180](https://github.com/toshiki670/merge-ready/pull/180)) ([`8ea56db`](https://github.com/toshiki670/merge-ready/commit/8ea56dbe0190d62fb8607473903e5b3373115446))
- CI チェック実行中（pending）のとき ⧖ wait-for-ci を表示する ([#181](https://github.com/toshiki670/merge-ready/pull/181)) ([`85a084a`](https://github.com/toshiki670/merge-ready/commit/85a084ae413f8676277097e7f8ebe3f03d321a5b))
- MergeStateStatus が UNKNOWN のとき ⧖ wait-for-status を表示する ([#182](https://github.com/toshiki670/merge-ready/pull/182)) ([`96b6ae8`](https://github.com/toshiki670/merge-ready/commit/96b6ae831c21a586bef62ca2a83cfdcab3caca98))
- [**BREAKING**] ラベルをセンテンスケースのアクション表現に統一し、review キーを changes_requested に改名 ([#185](https://github.com/toshiki670/merge-ready/pull/185)) ([`e2e5746`](https://github.com/toshiki670/merge-ready/commit/e2e5746108945dfd60f48fd2e22b06dd4e631f38))
- [**BREAKING**] [error] を単一セクションに統一し、静的 label をエラーメッセージで置換する ([#189](https://github.com/toshiki670/merge-ready/pull/189)) ([`4503387`](https://github.com/toshiki670/merge-ready/commit/4503387f02d3385c961aae7fff53f7ebe95bc3f5))


## [0.4.3] - 2026-04-27
### Features
- Draft PR のとき ✎ ready-for-review を表示する ([#173](https://github.com/toshiki670/merge-ready/pull/173)) ([`c639ecf`](https://github.com/toshiki670/merge-ready/commit/c639ecfa0f0a1dd73f62f1d677c5105f977c827a))


## [0.4.2] - 2026-04-27
### Features
- PR 未作成時に  を表示する ([#167](https://github.com/toshiki670/merge-ready/pull/167)) ([`c65a91e`](https://github.com/toshiki670/merge-ready/commit/c65a91e491d4c505ee0f9295af890e099747f364))


## [0.4.1] - 2026-04-26
### Bug Fixes
- Stale_delay_ms の初回 gh 呼び出しを即時化してフレーキーを解消 ([#152](https://github.com/toshiki670/merge-ready/pull/152)) ([`9bbd609`](https://github.com/toshiki670/merge-ready/commit/9bbd609e6316ded71ee4c61d04523c2c715af309))
### Features
- PR が closed/merged になったらリフレッシュを停止する ([#150](https://github.com/toshiki670/merge-ready/pull/150)) ([`192f153`](https://github.com/toshiki670/merge-ready/commit/192f153b767dd51939549b5fab324ef7a9a4b7e8))


## [0.4.0] - 2026-04-25
### Features
- [**BREAKING**] Config update サブコマンドと version フィールドの廃止、config edit → config への統合 ([#142](https://github.com/toshiki670/merge-ready/pull/142)) ([`4c8c9be`](https://github.com/toshiki670/merge-ready/commit/4c8c9beaa39c842e37a95aaaa5b20e2191625127))
- [**BREAKING**] Merge-ready-prompt 軽量バイナリの追加と prompt サブコマンドの削除 ([#147](https://github.com/toshiki670/merge-ready/pull/147)) ([`91d6bf4`](https://github.com/toshiki670/merge-ready/commit/91d6bf47cf76a6bfcb41d7737c1b8522d229b69c))


## [0.3.1] - 2026-04-24
### Bug Fixes
- Migrate app-id to client-id in release-prepare workflow ([#130](https://github.com/toshiki670/merge-ready/pull/130)) ([`c2ad532`](https://github.com/toshiki670/merge-ready/commit/c2ad53239efeab4045f0729bb28e2f24bda8fcb3))
### Features
- ErrorCategory / LogRecord の導入と simplelog による構造化ロギング ([#133](https://github.com/toshiki670/merge-ready/pull/133)) ([`da5b8ab`](https://github.com/toshiki670/merge-ready/commit/da5b8ab90f8e86112d57c76fb5ed5f5444ccddf4))


## [0.3.0] - 2026-04-24
### Features
- [**BREAKING**] Remove --no-cache option from prompt subcommand ([#125](https://github.com/toshiki670/merge-ready/pull/125)) ([`a46d89d`](https://github.com/toshiki670/merge-ready/commit/a46d89db104945b1d11986407af02b4a332fcdf8))


## [0.2.1] - 2026-04-22
### Bug Fixes
- Write error logs to merge-ready cache directory ([#115](https://github.com/toshiki670/merge-ready/pull/115)) ([`c344e30`](https://github.com/toshiki670/merge-ready/commit/c344e3003f845f08068dbe9ea6036c1ea093bbe4))


## [0.2.0] - 2026-04-21
### Bug Fixes
- Add timeout to gh command execution to prevent indefinite hang ([#84](https://github.com/toshiki670/merge-ready/pull/84)) ([`ab4f665`](https://github.com/toshiki670/merge-ready/commit/ab4f665aa2bf12d49f16c446e8c9244736055cd9))
- PRのないブランチで「? loading」が永続表示されるバグを修正 ([#90](https://github.com/toshiki670/merge-ready/pull/90)) ([`551080f`](https://github.com/toshiki670/merge-ready/commit/551080f895c607a36158c8c6c09cddfcb625ad18))
- Stabilize daemon startup and no-PR stale cache behavior ([#102](https://github.com/toshiki670/merge-ready/pull/102)) ([`3e95be7`](https://github.com/toshiki670/merge-ready/commit/3e95be70fe6cfdc71f95fcb0874fb1aab14a98ad))
- Restart daemon when prompt detects version mismatch ([#105](https://github.com/toshiki670/merge-ready/pull/105)) ([`cbea420`](https://github.com/toshiki670/merge-ready/commit/cbea42004d71870938297887602f343af24f87f5))
### Features
- [**BREAKING**] デーモン + Unix ソケットによる sub-ms キャッシュ応答（StatusCache BC） ([#75](https://github.com/toshiki670/merge-ready/pull/75)) ([`c2f9935`](https://github.com/toshiki670/merge-ready/commit/c2f99352589eeefb431564952e8e2a21228695d7))


## [0.1.2] - 2026-04-18
### Features
- Config edit / update サブコマンドを追加 ([#72](https://github.com/toshiki670/merge-ready/pull/72)) ([`257cb3e`](https://github.com/toshiki670/merge-ready/commit/257cb3ed51f68719951c09a77ae2ae33a8264fcd))


## [0.1.1] - 2026-04-18
### Bug Fixes
- Add toolchain input to dtolnay/rust-toolchain in CodeQL workflow ([`8074938`](https://github.com/toshiki670/merge-ready/commit/807493850b903e0a7f04b3d21b0d487c2a042e12))
- Change CodeQL Rust build-mode from manual to none ([`31bad17`](https://github.com/toshiki670/merge-ready/commit/31bad175817ce11fc21c7dd84083a0b456e25a01))
- Squash-merge 専用の commit_preprocessors Step 2 を削除 ([`510d2c7`](https://github.com/toshiki670/merge-ready/commit/510d2c7bf3d91c4c282eec4615cc1d18b21fb9c1))
- テスト環境から XDG_CONFIG_HOME を除去して設定読み込みを隔離 ([`cddef71`](https://github.com/toshiki670/merge-ready/commit/cddef71d179c16d19557df65df71cb7e358034c6))
- テスト環境で XDG_CONFIG_HOME を HOME/.config に固定 ([`2b2a3a3`](https://github.com/toshiki670/merge-ready/commit/2b2a3a37190eafdae81fc7cb351854bf2fd8894c))
- Release-plz で feat が minor バンプ・CHANGELOG に反映されない問題を修正 ([`64e4227`](https://github.com/toshiki670/merge-ready/commit/64e4227fee87e0b2528dd75d6aa655821e569ed0))
- Link_parsers を削除 ([#53](https://github.com/toshiki670/merge-ready/pull/53)) ([`2c6224e`](https://github.com/toshiki670/merge-ready/commit/2c6224e6ef16dfcbb53522459b0e52506194c23b))
- バージョンを 0.1.0 に戻す ([#59](https://github.com/toshiki670/merge-ready/pull/59)) ([`68a03a0`](https://github.com/toshiki670/merge-ready/commit/68a03a0809e4127520eeb46ccd1a86eda317b351))
- レイヤー依存違反を設計レベルで修正 ([`70afcef`](https://github.com/toshiki670/merge-ready/commit/70afcef2e3528ea1126eb585c210d1866f91d2e7))
### Features
- ~/.config/merge-ready.toml によるシンボル・フォーマットのカスタマイズ ([`f1b8759`](https://github.com/toshiki670/merge-ready/commit/f1b8759af2e0e45320129a39f97c9aadd32f294c))
- XDG_CONFIG_HOME に対応した設定ファイルパス解決 ([`73252a7`](https://github.com/toshiki670/merge-ready/commit/73252a7e3d4814660a9354c30eed3970fb14decd))
- 内部クレートの changelog_update を有効化 ([#57](https://github.com/toshiki670/merge-ready/pull/57)) ([`a7b7586`](https://github.com/toshiki670/merge-ready/commit/a7b75866ce22f18f085f70851df493159767cde1))
### Performance
- キャッシュパスを tmpfs（/tmp）に変更 ([`a79d6bc`](https://github.com/toshiki670/merge-ready/commit/a79d6bc59b6900907020a9e1f94928da2bcaca57))
- Git 子プロセスを廃止し .git ディレクトリ直接読み取りで repo_id を生成 ([#36](https://github.com/toshiki670/merge-ready/pull/36)) ([`cd86f2a`](https://github.com/toshiki670/merge-ready/commit/cd86f2adbdacce71e479a85bda0bf6425f2439f9))


## [0.1.0] - 2026-04-15
### Bug Fixes
- Treat "no checks reported" as empty CI checks instead of api-error ([`bfebf5d`](https://github.com/toshiki670/merge-ready/commit/bfebf5d6f9996e6ca5db335bc04a7ac422c762ce))
- Remove #[allow(dead_code)] from GhCheckItem.state ([`604750e`](https://github.com/toshiki670/merge-ready/commit/604750e4b587caa7bf7d34b293bac0ce62a0e8d8))
- No-args shows help; prompt subcommand required for PR status ([`98374e7`](https://github.com/toshiki670/merge-ready/commit/98374e7a0a42f54ad2135dd50f4b653a41908828))
- Use worktree path as cache key, flatten cache file structure ([`36a58c7`](https://github.com/toshiki670/merge-ready/commit/36a58c733147fa7ec1a3c34967adee3dacc7efcb))
- Pass PromptArgs by reference to satisfy clippy::needless_pass_by_value ([`eda26b3`](https://github.com/toshiki670/merge-ready/commit/eda26b36148fbb7c4ec48e5abef45f922426ae51))
- Prevent cache corruption on refresh error; deduplicate background spawns ([`f7f722a`](https://github.com/toshiki670/merge-ready/commit/f7f722aca5919ccf4f74694fa3b113eba434f8bc))
- Show nothing (not "? loading") when outside a git repository ([`e485791`](https://github.com/toshiki670/merge-ready/commit/e48579160b4014ba94a94ded3dcd01800ac76abd))
- Address PR review issues 1-3 ([`96d601e`](https://github.com/toshiki670/merge-ready/commit/96d601ec0e3b32250b49d8e673e438f9b31fce47))
- Address PR review issues 4-6 ([`625d7e7`](https://github.com/toshiki670/merge-ready/commit/625d7e74e25753d77a87ae186d7113871b596015))
- Simplify lock, include branch in cache key ([`2b44aa2`](https://github.com/toshiki670/merge-ready/commit/2b44aa2068d1f5964b372c478621089fd21edab4))
- Address PR review issues (PID reuse, atomic write, DDD layers) ([`165a3d8`](https://github.com/toshiki670/merge-ready/commit/165a3d83ceaa6a0fd93cd74dceb2d1ba09af9995))
- Eliminate empty-lock window and handle write failure in create_with_pid ([`9ae6f7b`](https://github.com/toshiki670/merge-ready/commit/9ae6f7b178907b04c3958e2f3e478ee1fc5c1fe8))
- Pass --repo-id to child, move spawn to CLI, use PID-based tmp ([`75d5aae`](https://github.com/toshiki670/merge-ready/commit/75d5aae77d9be695de35c4e63a18da7484be1f3b))
- Prevent code injection in dependabot-security workflow ([`54bf9e4`](https://github.com/toshiki670/merge-ready/commit/54bf9e4833281a0964f763839129a3f090aa9c57))
- Use GitHub App token in release-prepare to allow PR creation ([`d5e56c2`](https://github.com/toshiki670/merge-ready/commit/d5e56c2bfd3e336dcf84dbbb9fee2df6eddcf117))
- Align release-publish trigger with actual branch prefix ([`a5a5cb2`](https://github.com/toshiki670/merge-ready/commit/a5a5cb20cda2d321f14b40528af38a4d080cf882))
### Features
- Add E2E red tests and introduce rustfmt/clippy ([`011dfa1`](https://github.com/toshiki670/merge-ready/commit/011dfa160d2f30f12c256f522090e41e829e4ca8))
- Implement core PR merge readiness evaluation logic ([`e0d1a9d`](https://github.com/toshiki670/merge-ready/commit/e0d1a9d4e5998771da9e3b83a6b3eccca416b4c3))
- Detect update-branch via GitHub Compare API ([`20db7dd`](https://github.com/toshiki670/merge-ready/commit/20db7dd14ece9155b1aa69b9b597bc95cdc00caa))
- Sub-40ms latency via cache-first architecture (closes #7) ([`4216563`](https://github.com/toshiki670/merge-ready/commit/4216563356cb7fb753d1438f793711a49873800f))
- Add clap-based CLI with help and prompt subcommands (Issue #9) ([`a835a56`](https://github.com/toshiki670/merge-ready/commit/a835a560621c3c80318dc4528c1e33094f7a4946))
- Implement --refresh and --no-cache for prompt subcommand ([`a9992c5`](https://github.com/toshiki670/merge-ready/commit/a9992c5b0508eb95baa8a84e980df02153f8a778))
