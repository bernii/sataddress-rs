set dotenv-load

default:
    @just --list

run:
    @echo "Starting federation server for $DOMAINS, site name is $SITE_NAME$SITE_SUB_NAME"
    cargo run

fix:
    @echo "Will try to fix source files..."
    cargo +nightly fmt -v
    cargo clippy --fix

check:
    @echo "Checking if we're good to ship..."
    cargo clippy --locked -- -D warnings
    @echo "Checking done"

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
