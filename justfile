set dotenv-load := true

dev:
    cargo run -p pillbox-server

test:
    cargo test --workspace

check:
    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    npm run check

build:
    cargo build --locked --release -p pillbox-server

build-web:
    ./scripts/build-web.sh

schema-apply:
    atlas schema apply --url "$DATABASE_URL" --to file://db/schema.sql --dev-url 'docker://postgres/17/dev?search_path=public'

schema-check:
    ./scripts/schema-check.sh

network-test:
    npm test

docker-build:
    docker build -t pillbox .

docker-smoke:
    ./scripts/docker-smoke.sh
