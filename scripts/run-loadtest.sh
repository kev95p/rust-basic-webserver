#!/usr/bin/env bash
set -euo pipefail

# Rutas desde la raíz del proyecto.
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"

VUS="${VUS:-50}"
DURATION="${DURATION:-1m}"
RAMP_UP="${RAMP_UP:-30s}"
RAMP_DOWN="${RAMP_DOWN:-30s}"
NO_CONNECTION_REUSE="${NO_CONNECTION_REUSE:-false}"

SERVER_PID=""

cleanup() {
  if [[ -n "$SERVER_PID" ]] && kill -0 "$SERVER_PID" 2>/dev/null; then
    echo "→ Deteniendo servidor (PID $SERVER_PID)..."
    kill "$SERVER_PID" 2>/dev/null || true
    wait "$SERVER_PID" 2>/dev/null || true
  fi
}
trap cleanup EXIT

cd "$PROJECT_DIR"

# Decidimos si usamos k6 local o Docker antes de definir la URL por defecto.
if command -v k6 >/dev/null 2>&1; then
  USE_DOCKER=false
else
  USE_DOCKER=true
fi

if [[ -n "${BASE_URL:-}" ]]; then
  # El usuario ya definió BASE_URL; la usamos para el health check y para k6.
  HEALTH_URL="$BASE_URL"
  K6_BASE_URL="$BASE_URL"
elif [[ "$USE_DOCKER" == true ]]; then
  # Docker Desktop en macOS no puede usar localhost del host desde el contenedor.
  # host.docker.internal es la forma portable de llegar al host desde el container.
  HEALTH_URL="http://localhost:8080"
  K6_BASE_URL="http://host.docker.internal:8080"
else
  HEALTH_URL="http://localhost:8080"
  K6_BASE_URL="http://localhost:8080"
fi

echo "→ Compilando servidor en modo release..."
cargo build --release

echo "→ Iniciando servidor..."
./target/release/basic-webserver &
SERVER_PID=$!

echo "→ Esperando que el servidor esté listo en $HEALTH_URL..."
for _ in $(seq 1 30); do
  if curl -fs "$HEALTH_URL" >/dev/null 2>&1; then
    echo "→ Servidor listo."
    break
  fi
  sleep 0.5
done

if ! curl -fs "$HEALTH_URL" >/dev/null 2>&1; then
  echo "✗ El servidor no respondió a tiempo." >&2
  exit 1
fi

K6_ENV_ARGS=(
  -e "BASE_URL=$K6_BASE_URL"
  -e "VUS=$VUS"
  -e "DURATION=$DURATION"
  -e "RAMP_UP=$RAMP_UP"
  -e "RAMP_DOWN=$RAMP_DOWN"
  -e "NO_CONNECTION_REUSE=$NO_CONNECTION_REUSE"
)

if [[ "$USE_DOCKER" == false ]]; then
  echo "→ Ejecutando k6 local..."
  k6 run "${K6_ENV_ARGS[@]}" "$SCRIPT_DIR/loadtest.js"
else
  echo "→ k6 no encontrado localmente. Ejecutando con Docker..."
  docker run --rm \
    -v "$SCRIPT_DIR:/scripts" \
    -w /scripts \
    "${K6_ENV_ARGS[@]}" \
    grafana/k6:latest run "${K6_ENV_ARGS[@]}" /scripts/loadtest.js
fi
