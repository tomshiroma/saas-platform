# SaaS Platform

日本国内向けの一般業務SaaSを想定した設計資料です。

## Development

必要なソフトウェアはGit、Docker、Docker Composeです。

```bash
cp .env.example .env
make dev
```

起動後は以下へアクセスできます。

- Frontend: <http://localhost:5173>
- API health: <http://localhost:3000/api/v1/health/ready>
- OpenAPI: <http://localhost:3000/api/v1/openapi.json>
- Mailpit: <http://localhost:8025>

開発用管理者:

- テナントID: `development`
- メールアドレス: `admin@example.test`
- パスワード: `development-password`

実装済みの初期機能:

- テナント新規登録と初期管理者作成
- テナントID・メールアドレス・パスワードによるログイン
- HttpOnly Cookieセッション、CSRF対策、ログアウト
- メールによるパスワード再設定
- 管理者によるテナント名変更
- テナントメンバーの追加、権限変更、有効・無効化、削除
- PostgreSQL RLSによるテナントデータ分離

MinIOを含めて起動する場合は`make dev-tools`を使用します。

主要コマンド:

```bash
make migrate
make seed
make lint
make test
make down
```

## Documents

- [作業ガイド](AGENTS.md)
- [非機能要件定義書](docs/non-functional-requirements.md)
- [技術スタック・初期インフラ構成](docs/technology-stack.md)
- [開発・検証環境設計](docs/environments.md)
