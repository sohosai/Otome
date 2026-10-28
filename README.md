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

## 使い方

このソフトウェアは **ビルド済みの `otome.exe` を使います。**
ZIP を解凍して `start.cmd` を実行するだけで、NDI Runtime の確認から起動まで行います。
ソースからビルドする方法は[開発者向け](#開発者向け)にまとめています。

### 必要なもの

| 項目 | 要件 |
| --- | --- |
| OS | Windows x64 |
| NDI | NDI Runtime 5 または 6（[NDI Tools](https://ndi.video/tools/) に含まれます） |
| OBS（任意） | NDI 出力に DistroAV などの NDI プラグイン |

### 1. ダウンロード

[GitHub Releases](https://github.com/sohosai/Otome/releases) から
`otome-v<version>-windows-x64.zip` をダウンロードし、任意のフォルダに解凍します。

ZIP に含まれるもの:

- `otome.exe`
- `start.cmd`（NDI Runtime の DLL を探して `otome.exe` を起動するランチャー）
- `run.ps1`（PowerShell 版ランチャー。ExecutionPolicy の制限がある場合は `start.cmd` を使用）
- `config.example.toml`
- `README.md`

NDI Runtime が未インストールの場合は、先に [NDI Tools](https://ndi.video/tools/) をインストールしてください。

### 2. 設定ファイルを作成

解凍したフォルダで、設定の雛形を `otome.toml` にコピーします。

```powershell
Copy-Item config.example.toml otome.toml
```

少なくとも `[ndi]` の `source_name` を、実際に受信する NDI ソース名に合わせます。

```toml
[ndi]
source_name = "OBS PGM"
output_name = "OTOME-OUT"

[mute]
mode = "api"
listen = "0.0.0.0:7878"
```

設定ファイルが無い場合はすべてデフォルト値で動作します。全項目は[設定](#設定)を参照してください。

### 3. OBS 側で NDI 出力を有効にする

DistroAV の設定（ツール → DistroAV NDI Settings）で **Main Output** を有効にし、
NDI 名を設定します（例: `OBS PGM`）。OBS のプログラム映像と音声が NDI で送出されます。

NDI 上では `ホスト名 (OBS PGM)` のように見えます。Otome の `source_name` は
`OBS PGM` のような短い名前でも、`ホスト名 (OBS PGM)` のような完全な名前でも一致します。

### 4. 起動

解凍したフォルダで `start.cmd` をダブルクリックするか、ターミナルで実行します。

```powershell
.\start.cmd
```

`start.cmd` は NDI Runtime を確認して `PATH` に追加し、カレントディレクトリの `otome.toml` を
読み込んで `otome.exe` を起動します。`otome.toml` が無い場合はデフォルト設定で起動します。

ログに次の順で出れば動作しています。

```
starting NDI proxy
NDI sender ready
OBS NDI source found source=...
NDI receiver connected
```

停止は `Ctrl+C` です。エクスプローラーから `start.cmd` をダブルクリックしても起動できますが、
エラー内容を確認できるターミナルからの起動がおすすめです。

### 5. 出力 NDI ソースを受け取る

別 PC の OBS、または NDI Studio Monitor で `OTOME-OUT` を受信します。
映像が表示され、音声メーターが動けば転送成功です。

> 注意: 送出元と同じ OBS のプログラムに `OTOME-OUT` を追加すると映像・音声がループします。
> 確認には別 PC か、2 つ目の OBS インスタンス、NDI Studio Monitor を使ってください。

### 6. ミュートを切り替える

API モード（`mode = "api"`）では `http://127.0.0.1:7878` で待ち受けます。

```powershell
# 状態取得
Invoke-RestMethod http://127.0.0.1:7878/mute

# ミュート ON（音声だけ止まる）
Invoke-RestMethod -Method Post -Uri http://127.0.0.1:7878/mute `
  -ContentType 'application/json' -Body '{"is_muted":true}'

# ミュート OFF
Invoke-RestMethod -Method Post -Uri http://127.0.0.1:7878/mute `
  -ContentType 'application/json' -Body '{"is_muted":false}'
```

Otome のログに `mute on: NDI audio is gated` / `mute off: NDI audio is forwarded` が出れば
状態が切り替わっています。映像はミュート中も流れ続けます。

## 設定

`start.cmd` はカレントディレクトリの `otome.toml` を自動で読み込みます。`start.cmd` を
ダブルクリックした場合は `otome.exe` と同じフォルダの `otome.toml` が使われます。
ファイルが無い場合はすべてデフォルト値で動作します。

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

`forward_to_remote = true` を使う場合、転送先には自分自身とは別のホスト・ポートを指定してください。
`listen` と同じアドレスを指定すると自分自身へ転送し続けるループになります。

### `[log]`

| キー | デフォルト | 説明 |
| --- | --- | --- |
| `level` | `"info"` | ログレベル。`RUST_LOG` 環境変数が優先される |

## ミュート状態の取得モード

### `mode = "api"`

Otome 自身が HTTP API を立てます。外部から `POST /mute` して状態を切り替えます。
`forward_to_remote = true` にすると、受け取った状態を `remote_url` の `/mute` にも転送します。

### `mode = "poll"`

`remote_url` の `/mute` を `poll_interval_ms` 間隔で取得し、`{"is_muted": bool}` を読み取ります。
このモードではローカル API は起動しません。

```toml
[mute]
mode = "poll"
remote_url = "http://192.168.1.10:7878"
poll_interval_ms = 200
```

## HTTP API

`mode = "api"` のとき、`listen` で待ち受けます（CORS は全許可）。

| メソッド | パス | 説明 |
| --- | --- | --- |
| `GET` | `/health` | `{"ok": true}` を返す |
| `GET` | `/mute` | `{"is_muted": false}` を返す |
| `POST` | `/mute` | `{"is_muted": true}` を受け付けて状態を更新 |

```powershell
# 疎通確認
Invoke-RestMethod http://127.0.0.1:7878/health

# 状態取得
Invoke-RestMethod http://127.0.0.1:7878/mute

# ミュート ON
Invoke-RestMethod -Method Post -Uri http://127.0.0.1:7878/mute `
  -ContentType 'application/json' -Body '{"is_muted":true}'

# ミュート OFF
Invoke-RestMethod -Method Post -Uri http://127.0.0.1:7878/mute `
  -ContentType 'application/json' -Body '{"is_muted":false}'
```

## トラブルシューティング

### NDI ソースが見つからない

- OBS の NDI 出力が NDI Studio Monitor から見えるか確認する
- OBS 側で DistroAV の Main Output が有効になっているか確認する
- Otome と OBS が同じネットワークにいるか確認する
- Windows ファイアウォールで NDI / Otome を許可する
- NDI Runtime（NDI Tools）がインストールされているか確認する

### 出力が真っ暗 / 映像が出ない

- 受信側で `OTOME-OUT` ではなく `OBS PGM` を直接選んで映るか確認する
  - それも映らない場合は OBS の NDI 出力側の問題
  - `OBS PGM` は映るのに `OTOME-OUT` が映らない場合は Otome の転送側の問題
- Otome のログに `NDI sender ready` と `NDI receiver connected` が出ているか確認する
- 受信側で `OTOME-OUT` を追加したときに `connections=1` のログが出るか確認する
- 受信側でソースを追加したあと Otome を再起動する
- `otome.toml` で一時的に `color_format = "fastest"`、`clock_video = false` を試す

### DLL が見つからない / 起動できない

- NDI Runtime（NDI Tools）をインストールする
- `start.cmd` 経由で起動する
- `Processing.NDI.Lib.x64.dll` のあるディレクトリを `PATH` に追加する

### `run.ps1` が実行できない

ダウンロードしたスクリプトは Windows にブロックされていることがあります。

```powershell
Unblock-File .\run.ps1
.\run.ps1
```

実行ポリシーを変更できない場合は、プロセス単位で回避するか `start.cmd` を使います。

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\run.ps1
```

### ポートが使用中 / API に接続できない

- `otome.toml` の `[mute] listen` を別のポートに変える
- `mode = "api"` になっているか確認する（`poll` ではローカル API は起動しない）
- `GET /health` で疎通確認する

## 開発者向け

ここから先は Otome 自体をビルド・改修する人向けです。通常の利用には Rust は不要です。

### 必要環境

| 項目 | 要件 |
| --- | --- |
| OS | Windows x64（`run.ps1` は Windows 向け。NDI クレート自体は Linux x64 も対象） |
| Rust | 1.88 以上（edition 2024） |
| NDI | NDI Runtime 5 または 6 |

`ndi` クレートが SDK のライブラリを同梱しているため、通常は追加の SDK インストールなしでビルドできます。

### クローンとビルド

```powershell
git clone https://github.com/sohosai/Otome.git
cd Otome
cargo build --release
```

ビルドしたバイナリは `target\release\otome.exe` にできます。
開発中に直接起動する場合は `cargo run` を使います。

```powershell
cargo run --release
cargo run --release -- --list-sources
cargo run --release -- --source-name "OBS PGM"
```

`otome.exe` / `cargo run` で使えるオプション:

| オプション | 説明 |
| --- | --- |
| `-c`, `--config <PATH>` | 設定ファイル（デフォルト `otome.toml`） |
| `--source-name <NAME>` | `ndi.source_name` を上書き |
| `--output-name <NAME>` | `ndi.output_name` を上書き |
| `--mute-listen <ADDR>` | `mute.listen` を上書き |
| `--list-sources` | NDI ソースを一覧表示して終了 |
| `--dry-run` | NDI プロキシを起動しない |

`run.ps1` を使うと、ビルド済みの `otome.exe`（`target\release` / `target\debug`）を探して起動し、
NDI Runtime の DLL があるディレクトリを `PATH` に追加します。

### テスト

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

### リリース

`v*` 形式のタグを push すると GitHub Actions が Windows x64 版をビルドし、
GitHub Release に ZIP と SHA256 を自動で添付します。

```powershell
git tag v0.1.0
git push origin v0.1.0
```

リリースワークフローは `.github/workflows/release.yml` にあります。

## 構成

| ファイル | 役割 |
| --- | --- |
| `src/main.rs` | CLI 解析、設定読み込み、起動・終了処理 |
| `src/config.rs` | 設定型・デフォルト値・バリデーション・テスト |
| `src/mute.rs` | HTTP API サーバーと外部 API ポーリング |
| `src/ndi_proxy.rs` | NDI ソース検索・再接続・映像/音声転送ループ・テスト |
| `src/ndi_raw.rs` | `ndi` クレートの低レベル API ラッパー（Finder/Recv/Sender） |
| `start.cmd` | 配布版の起動と NDI Runtime の `PATH` 設定（ExecutionPolicy 非依存） |
| `run.ps1` | PowerShell 版の起動スクリプトと NDI Runtime の `PATH` 設定 |
| `config.example.toml` | 設定ファイルの雛形 |
| `.github/workflows/release.yml` | `v*` タグで Windows 向け ZIP をビルドし Release に添付 |
