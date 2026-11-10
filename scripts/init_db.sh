#!/usr/bin/env bash
if ! [ -x "$(command -v sqlx)" ]; then
  echo >&2 "Error: sqlx is not installed."
  echo >&2 "Use:"
  echo >&2 " cargo install --version='~0.8' sqlx-cli \\
--no-default-features --features rustls,postgres"
  echo >&2 "to install it."
  exit 1
fi

set -x
set -eo pipefail

# Use these defaults unless overridden in the environment.
DB_PORT="${POSTGRES_PORT:=5432}"
SUPERUSER="${SUPERUSER:=postgres}"
SUPERUSER_PWD="${SUPERUSER_PWD:=password}"
APP_USER="${APP_USER:=app}"
APP_USER_PWD="${APP_USER_PWD:=secret}"
APP_DB_NAME="${APP_DB_NAME:=newsletter}"
CONTAINER_NAME="${CONTAINER_NAME:=postgres}"

if [[ "$(docker inspect -f '{{.State.Running}}' "${CONTAINER_NAME}" 2>/dev/null || true)" == "true" ]]; then
  >&2 echo "Postgres container ${CONTAINER_NAME} is already running."
else
  # Launch PostgreSQL in Docker with extra connections for testing.
  docker run \
    --name "${CONTAINER_NAME}" \
    --detach \
    --publish "${DB_PORT}:5432" \
    --env "POSTGRES_USER=${SUPERUSER}" \
    --env "POSTGRES_PASSWORD=${SUPERUSER_PWD}" \
    --health-cmd="pg_isready -U ${SUPERUSER} || exit 1" \
    --health-interval=1s \
    --health-timeout=5s \
    --health-retries=5 \
    postgres -N 1000

  # Wait for PostgreSQL to be ready to accept connections.
  until [[ "$(docker inspect -f '{{.State.Health.Status}}' "${CONTAINER_NAME}")" == "healthy" ]]; do
    >&2 echo "Postgres is still unavailable - sleeping"
    sleep 1
  done
  >&2 echo "Postgres is up and running on port ${DB_PORT}!"

fi

# Create the application user if it does not already exist. This also lets a
# setup interrupted after starting the container be retried.
if ! docker exec "${CONTAINER_NAME}" psql -U "${SUPERUSER}" -tAc "SELECT 1 FROM pg_roles WHERE rolname = '${APP_USER}'" | grep -qx 1; then
  CREATE_QUERY="CREATE USER ${APP_USER} WITH PASSWORD '${APP_USER_PWD}';"
  docker exec "${CONTAINER_NAME}" psql -U "${SUPERUSER}" -c "${CREATE_QUERY}"
fi
GRANT_QUERY="ALTER USER ${APP_USER} CREATEDB;"
docker exec "${CONTAINER_NAME}" psql -U "${SUPERUSER}" -c "${GRANT_QUERY}"

DATABASE_URL="postgres://${APP_USER}:${APP_USER_PWD}@localhost:${DB_PORT}/${APP_DB_NAME}"
export DATABASE_URL
sqlx database create
sqlx migrate run
>&2 echo "Postgres has been migrated, ready to go!"

