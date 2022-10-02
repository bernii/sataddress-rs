default:
    @just --list

dev:
    RUST_BACKTRACE=1 DOMAINS=extensa.pl,sats.rs PIN_SECRET=my-secret-phrase SITE_NAME=SATADDRESS cargo run

build:
    @echo "Building the binary..."
    cargo +nightly fmt -v
    cargo clippy --fix
    @echo "binaries ready"

test:
    @echo "TODO: testing not implemented yet!"

db_init:
    @echo "Initializing the db with data..."

deploy:
    @echo "[0/5] Preparing the app for a deploy..."
    @echo "[1/5] Building the app"
    @echo "[2/5] Building the docker container"
    @echo "[3/5] Pushing to registry"

    @echo "[4/5] Deploying to remote host"

    @echo "[5/5] Running post-deploy verification tests",
    @echo "[6/6] All done!"
