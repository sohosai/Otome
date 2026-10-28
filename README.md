# Otome

NDI の音声を「ミュート」状態に応じてゲートするプロキシです。OBS などの NDI ソースを受け取り、
映像は常にそのまま、音声はミュートされていないときだけ別の NDI ソースとして再送信します。

```
OBS (NDI)  ──video/audio──▶  Otome  ──video + audio(未ミュート時)──▶  OBS/視聴側
                                  ▲
                                  │ HTTP API or 外部 API をポーリング
                             ミュート状態
```

## 主な機能

- NDI ソースを検索して自動接続（完全一致 → 部分一致の順）。切断されたら自動再接続。
- 映像は常に転送。音声はミュート中は破棄（`drop`）または無音フレームに差し替え（`silence`）。
- ミュート状態の取得方法は 2 通り
  - `api`: 本アプリが HTTP API を立て、`POST /mute` で状態を受け付ける
  - `poll`: 外部の `/mute` を一定間隔でポーリングする
- 出力 NDI ソース名・グループ、帯域、色形式、クロックなどを設定可能。
- `--dry-run` で NDI を触らずに API 部分だけ起動可能。

## 必要環境

- Windows x64（`run.ps1` は Windows 向け。NDI クレート自体は Linux x64 も対象）
- Rust 1.88 以上（edition 2024）
- NDI Runtime 5 または 6（`Processing.NDI.Lib.x64.dll`）

`ndi` クレートが SDK のライブラリを同梱しているため、通常は追加の SDK インストールなしでビルドできます。
実行時に DLL が見つからない場合は、NDI Runtime のインストール先を `PATH` に追加するか、
`run.ps1` 経由で起動してください（既知のインストール先を自動で `PATH` に追加します）。

## ビルド

```powershell
cargo build --release
```

## 設定

`otome.toml` をカレントディレクトリに置くと読み込まれます。ファイルが無い場合はすべてデフォルト値で動作します。
雛形は `config.example.toml` を参照してください。

```powershell
Copy-Item config.example.toml otome.toml
```

### `[ndi]`

| キー | デフォルト | 説明 |
| --- | --- | --- |
| `source_name` | `"OBS"` | 受信する NDI ソース名。完全一致または部分一致で検索 |
| `output_name` | `"OTOME-OUT"` | 送信する NDI ソース名。自分自身は検索対象から除外 |
| `output_groups` | `"Public"` | 出力ソースのグループ（空文字で未指定） |
| `bandwidth` | `"highest"` | `highest` / `lowest` / `audio_only` / `metadata_only` |
| `color_format` | `"uyvy_bgra"` | `bgrx_bgra` / `uyvy_bgra` / `rgbx_rgba` / `uyvy_rgba` / `fastest` / `best` |
| `clock_video` | `true` | 送信側で映像クロックを生成する |
| `clock_audio` | `false` | 送信側で音声クロックを生成する |
| `capture_timeout_ms` | `500` | 受信フレーム取得のタイムアウト |
| `reconnect_delay_ms` | `1000` | 再接続・リトライ間隔（最小 100） |
| `status_interval_ms` | `5000` | 接続数ログの出力間隔（最小 1000） |

### `[mute]`

| キー | デフォルト | 説明 |
| --- | --- | --- |
| `mode` | `"api"` | `api` または `poll` |
| `listen` | `"0.0.0.0:7878"` | API モードの待ち受けアドレス |
| `remote_url` | `"http://127.0.0.1:7878"` | 外部 API のベース URL（転送先・ポーリング先） |
| `forward_to_remote` | `false` | API で受けたミュート状態を外部 API にも転送する |
| `poll_interval_ms` | `200` | 外部 API のポーリング間隔（最小 50） |
| `audio_mode` | `"drop"` | ミュート中の音声処理。`drop`（破棄）または `silence`（無音送信） |

### `[log]`

| キー | デフォルト | 説明 |
| --- | --- | --- |
| `level` | `"info"` | ログレベル。`RUST_LOG` 環境変数が優先される |

## 起動

```powershell
# 設定ファイルを指定して起動
cargo run --release

# 設定ファイルを指定
cargo run --release -- --config .\otome.toml

# 一部の設定をコマンドラインで上書き
cargo run --release -- --source-name "OBS-PGM" --output-name "OTOME-OUT"

# 利用可能な NDI ソースを一覧表示
cargo run --release -- --list-sources

# NDI を起動せず HTTP API のみ確認
cargo run --release -- --dry-run
```

`run.ps1` を使うと、ビルド済みの `otome.exe`（`target\release` / `target\debug`）を探して起動し、
NDI Runtime の DLL があるディレクトリを `PATH` に追加します。

```powershell
.\run.ps1
.\run.ps1 --list-sources
```

### CLI オプション

| オプション | 説明 |
| --- | --- |
| `-c`, `--config <PATH>` | 設定ファイル（デフォルト `otome.toml`） |
| `--source-name <NAME>` | `ndi.source_name` を上書き |
| `--output-name <NAME>` | `ndi.output_name` を上書き |
| `--mute-listen <ADDR>` | `mute.listen` を上書き |
| `--list-sources` | NDI ソースを一覧表示して終了 |
| `--dry-run` | NDI プロキシを起動しない |

## HTTP API

`mode = "api"` のとき、`listen` で待ち受けます（CORS は全許可）。

| メソッド | パス | 説明 |
| --- | --- | --- |
| `GET` | `/health` | `{"ok": true}` を返す |
| `GET` | `/mute` | `{"is_muted": false}` を返す |
| `POST` | `/mute` | `{"is_muted": true}` を受け付けて状態を更新 |

```powershell
# 状態取得
Invoke-RestMethod http://127.0.0.1:7878/mute

# ミュート ON
Invoke-RestMethod -Method Post -Uri http://127.0.0.1:7878/mute `
  -ContentType 'application/json' -Body '{"is_muted":true}'

# ミュート OFF
Invoke-RestMethod -Method Post -Uri http://127.0.0.1:7878/mute `
  -ContentType 'application/json' -Body '{"is_muted":false}'
```

## テスト

ユニットテストは NDI を起動せずに実行できます。

```powershell
# 全テスト
cargo test

# 設定まわりのみ
cargo test config::

# ソース名マッチングのみ
cargo test ndi_proxy::
```

現在のテスト内容:

- `config::tests::defaults_are_valid` — デフォルト設定がバリデーションを通ること
- `config::tests::parses_modes_and_bandwidth` — TOML から各 enum が正しくパースされること
- `ndi_proxy::tests::matches_ndi_source_names` — NDI ソース名の完全一致・部分一致・自己除外

### 手動での動作確認

1. OBS などで NDI 出力を有効にする。
2. `cargo run -- --list-sources` でソース名を確認し、必要なら `--source-name` を合わせる。
3. `cargo run -- --dry-run` で API の疎通確認。

   ```powershell
   Invoke-RestMethod http://127.0.0.1:7878/health
   ```

4. `cargo run` で本起動し、出力 NDI ソース（デフォルト `OTOME-OUT`）を OBS などで受信する。
5. `POST /mute` で `is_muted` を切り替え、音声だけが止まることを確認する。
   `audio_mode = "silence"` の場合は無音が流れ続けます。

## 構成

| ファイル | 役割 |
| --- | --- |
| `src/main.rs` | CLI 解析、設定読み込み、起動・終了処理 |
| `src/config.rs` | 設定型・デフォルト値・バリデーション・テスト |
| `src/mute.rs` | HTTP API サーバーと外部 API ポーリング |
| `src/ndi_proxy.rs` | NDI ソース検索・再接続・映像/音声転送ループ・テスト |
| `src/ndi_raw.rs` | `ndi` クレートの低レベル API ラッパー（Finder/Recv/Sender） |
