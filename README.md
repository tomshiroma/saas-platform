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

MailpitとMinIOを含めて起動する場合は`make dev-tools`を使用します。

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
