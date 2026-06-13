COMPOSE := docker compose -f compose.yaml -f compose.dev.yaml

.PHONY: dev dev-tools down reset migrate seed test test-frontend test-backend e2e lint logs ps

dev:
	$(COMPOSE) up --build

dev-tools:
	$(COMPOSE) --profile tools up --build

down:
	$(COMPOSE) --profile tools down

reset:
	$(COMPOSE) --profile tools down --volumes
	$(COMPOSE) up -d postgres
	$(COMPOSE) run --rm migrate
	$(COMPOSE) run --rm api cargo run -- seed

migrate:
	$(COMPOSE) up -d postgres
	$(COMPOSE) run --rm migrate

seed:
	$(COMPOSE) up -d postgres
	$(COMPOSE) run --rm migrate
	$(COMPOSE) run --rm api cargo run -- seed

test: test-frontend test-backend

test-frontend:
	$(COMPOSE) run --rm --no-deps frontend pnpm test --run

test-backend:
	$(COMPOSE) run --rm api cargo test

e2e:
	@echo "Playwrightは主要業務フローの実装時に追加します。"

lint:
	$(COMPOSE) run --rm --no-deps frontend pnpm lint
	$(COMPOSE) run --rm --no-deps frontend pnpm typecheck
	$(COMPOSE) run --rm --no-deps api cargo fmt --check
	$(COMPOSE) run --rm --no-deps api cargo clippy --all-targets --all-features -- -D warnings

logs:
	$(COMPOSE) logs -f

ps:
	$(COMPOSE) ps

