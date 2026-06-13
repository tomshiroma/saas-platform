# SaaS Platform 作業ガイド

このファイルはリポジトリ全体に適用する。サブディレクトリに別の`AGENTS.md`が追加された場合、その配下ではより具体的な指示を優先する。

## 1. 現在の状態

このリポジトリは初期開発段階にある。React、Rust、PostgreSQLの最小アプリケーションとDocker Compose開発環境は実装済みである。検証・本番構成とCI/CDはまだ実装されていない。存在しないコマンドや構成を実装済みとして扱わない。

作業前に、変更内容に応じて次の文書を確認する。

- [非機能要件定義書](docs/non-functional-requirements.md)
- [技術スタック・初期インフラ構成](docs/technology-stack.md)
- [開発・検証環境設計](docs/environments.md)

コードと文書が矛盾する場合は、黙って一方へ合わせない。要件変更として影響範囲を確認し、関連文書も同時に更新する。

## 2. 確定技術

- フロントエンド: React、TypeScript、Vite、React Router、TanStack Query、MUI
- バックエンド: Rust stable、Axum、Tokio、Serde、SQLx、utoipa
- データベース: PostgreSQL
- API: REST、OpenAPI 3.1、`/api/v1`
- ローカル開発: Docker Compose
- 検証・初期本番: ConoHa VPS上のDocker Compose
- リバースプロキシ: Caddy
- アーキテクチャ: モジュラモノリス

新しいフレームワーク、データストア、常駐サービスを追加する前に、既存技術で解決できない理由、運用負荷、VPSのリソース消費、障害時の復旧方法を明確にする。

## 3. 実装原則

### フロントエンド

- TypeScriptの厳格な型検査を維持し、安易に`any`を使用しない。
- サーバー状態はTanStack Query、入力フォームはReact Hook FormとZodを標準とする。
- APIクライアントと型はOpenAPIから生成し、手書き型との重複を避ける。
- 認証トークンをLocal Storageへ保存しない。
- ローディング、空状態、エラー、権限不足を明示的に表示する。
- 主要フローはキーボード操作と一般的な支援技術を考慮する。
- React作業では`.agents/skills/vercel-react-best-practices/SKILL.md`の関連項目を確認する。ただしNext.jsまたはSSR固有の規則は、このSPAへ無条件に適用しない。

### バックエンド

- 業務領域ごとにモジュールを分け、HTTP、ユースケース、ドメイン、永続化の責務を混在させない。
- 入力値、認証、認可、テナント境界はサーバー側で必ず検証する。
- エラー応答は`code`、`message`、`request_id`を持つ共通形式とし、SQLや内部エラーを公開しない。
- 時刻はUTCで保存・交換し、画面表示時にJSTへ変換する。
- 外部公開する識別子にはUUID v7を標準とする。
- ログは`tracing`による構造化JSONとし、リクエストIDとテナントIDを追跡可能にする。
- パスワードはArgon2idで保存し、秘密情報、トークン、不要な個人情報をログへ出力しない。

### データベース

- 共有DB・共有スキーマ方式とし、すべてのテナント所有データに`tenant_id`を持たせる。
- アプリケーション認可とPostgreSQL RLSの両方でテナント境界を保護する。
- 接続プールへ前リクエストのテナントコンテキストを残さない。
- 主キー、外部キー、一意制約、NOT NULLなど、DBで保証できる整合性はDB制約でも保証する。
- 既に共有されたmigrationを書き換えず、新しいmigrationを追加する。
- DB変更はexpand-and-contract方式を優先し、アプリケーションのロールバックを阻害しない。
- 監査対象操作は追記専用監査ログへ、操作者、テナント、対象、操作、結果、時刻、リクエストIDを記録する。

## 4. セキュリティ・データ管理

- 最小権限、RBAC、管理者MFAを前提に設計する。
- CookieはHttpOnly、SameSiteを設定し、検証・本番ではSecureを必須とする。
- 状態変更APIにCSRF対策を適用する。
- PostgreSQL、内部API、メトリクス用ポートをインターネットへ公開しない。
- 秘密情報をGit、Dockerイメージ、Compose定義、ログへ含めない。
- `.env.example`には安全なダミー値だけを置き、実値ファイルをコミットしない。
- 本番データを開発環境へ持ち込まない。
- 検証環境では生成データを使用する。本番由来データが不可欠な場合は、承認、仮名化、期限付きアクセス、利用後削除を必須とする。
- 開発、検証、本番で、秘密情報、外部サービスアカウント、DB、オブジェクトストレージを共有しない。
- 顧客ファイルをVPSローカルディスクへ永続保存しない。

## 5. 開発環境

開発端末に必要なソフトウェアはGit、Docker EngineまたはDocker Desktop、Docker Composeのみとする。Node.js、pnpm、Rust、Cargo、PostgreSQLはコンテナ内で実行する。

予定する標準サービスは`frontend`、`api`、`postgres`、`migrate`である。Mailpit、MinIO、Playwrightは必要時にCompose profileで起動する。

以下を標準コマンドとする。

| コマンド | 目的 |
|---|---|
| `make dev` | 開発環境を起動する |
| `make down` | コンテナを停止する |
| `make reset` | 開発データを破棄し、migrationとseedから再作成する |
| `make migrate` | migrationを適用する |
| `make seed` | 再実行可能な開発データを投入する |
| `make test` | フロントエンドとバックエンドのテストを実行する |
| `make e2e` | PlaywrightのE2Eテストを実行する |
| `make lint` | TypeScriptとRustの静的検査を実行する |

Playwrightのシナリオは未実装であり、現時点の`make e2e`はその旨を表示する。

## 6. テスト方針

変更のリスクと影響範囲に応じて、次を実施する。

- React: lint、型検査、Vitest、Testing Library
- Rust: `cargo fmt --check`、Clippy、単体テスト
- DB: 空DBへの全migration、更新migration、PostgreSQLを使った統合テスト
- API: 認証、認可、入力検証、共通エラー形式
- マルチテナント: 別テナント参照の拒否、RLS、接続プールのコンテキスト漏えい
- E2E: ログイン、主要業務フロー、管理者と一般利用者の権限差
- コンテナ: build、起動、healthcheck

不具合修正には、可能な限り再現テストを先に追加する。テストを実行できない場合は、理由と未確認のリスクを作業結果へ記載する。

## 7. 変更完了基準

- 変更した振る舞いをテストで確認している。
- formatter、lint、型検査が通っている。
- API変更時にOpenAPIと生成クライアントを更新している。
- DB変更時にmigration、ロールバック可能性、テナント分離を確認している。
- 構成、運用、要件が変わった場合に関連文書を更新している。
- 新しい環境変数を`.env.example`と設定説明へ反映している。
- 秘密情報や本番データが差分へ含まれていない。
- `git diff --check`で空白エラーがない。
- 利用可能になった標準コマンドがAGENTS.mdの記載と一致している。

## 8. 禁止事項

- 本番または検証サーバー上でソースコードをビルドしない。
- 本番配備に`latest`タグを使用しない。検証済みの同一イメージdigestを本番へ昇格する。
- migrationや本番データを安易に削除・初期化しない。
- テナントIDをクライアント入力だけから信頼しない。
- 認証・認可・監査をフロントエンドだけで実装しない。
- セキュリティ検査や失敗したテストを無効化して変更を通さない。
- 関係のないリファクタリング、依存関係更新、整形を同じ変更へ混在させない。
- Kubernetes、マイクロサービス、Redis、Elasticsearch、GraphQL、Next.jsを、設計文書にある再検討条件を満たさず導入しない。
