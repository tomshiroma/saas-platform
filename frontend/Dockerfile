FROM node:24.16.0-bookworm-slim AS development

ENV PNPM_HOME=/pnpm
ENV PATH=$PNPM_HOME:$PATH

RUN corepack enable \
    && corepack prepare pnpm@11.6.0 --activate \
    && useradd --create-home --uid 10001 app

WORKDIR /app
RUN chown app:app /app

COPY --chown=app:app package.json pnpm-lock.yaml pnpm-workspace.yaml ./
USER app
RUN pnpm install --frozen-lockfile

COPY --chown=app:app . .

EXPOSE 5173

CMD ["pnpm", "dev", "--host", "0.0.0.0"]

FROM development AS build
RUN pnpm build

FROM caddy:2.10.2-alpine AS runtime
COPY Caddyfile /etc/caddy/Caddyfile
COPY --from=build /app/dist /srv
