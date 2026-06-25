import http from 'k6/http';
import { check, sleep } from 'k6';

const BASE_URL = __ENV.BASE_URL || 'http://localhost:8080';
const VUS = parseInt(__ENV.VUS || '50', 10);
const DURATION = __ENV.DURATION || '1m';
const RAMP_UP = __ENV.RAMP_UP || '30s';
const RAMP_DOWN = __ENV.RAMP_DOWN || '30s';
const SLEEP_MIN = parseFloat(__ENV.SLEEP_MIN || '0.5');
const SLEEP_MAX = parseFloat(__ENV.SLEEP_MAX || '2');
const NO_CONNECTION_REUSE = (__ENV.NO_CONNECTION_REUSE || 'false').toLowerCase() === 'true';

const ENDPOINTS = [
  { path: '/', name: 'home' },
  { path: '/home', name: 'home_alias' },
  { path: '/index.html', name: 'index_html' },
  { path: '/styles.css', name: 'styles_css' },
  { path: '/app.js', name: 'app_js' },
  { path: '/test.html', name: 'test_html' },
  { path: '/test.txt', name: 'test_txt' },
  { path: '/desconocido', name: 'not_found', expected: 404 },
];

function randomIntBetween(min, max) {
  return Math.floor(Math.random() * (max - min + 1)) + min;
}

function randomSleep() {
  sleep(randomIntBetween(SLEEP_MIN, SLEEP_MAX));
}

function hitEndpoint(endpoint) {
  const url = `${BASE_URL}${endpoint.path}`;
  const res = http.get(url, { tags: { name: endpoint.name } });

  const expectedStatus = endpoint.expected || 200;
  check(res, {
    [`${endpoint.name} status is ${expectedStatus}`]: (r) =>
      r.status === expectedStatus,
    [`${endpoint.name} response time < 500ms`]: (r) =>
      r.timings.duration < 500,
  });
}

function hitRandomEndpoint() {
  const endpoint = ENDPOINTS[Math.floor(Math.random() * ENDPOINTS.length)];
  hitEndpoint(endpoint);
}

export const options = {
  // Desactivar reutilización de conexiones elimina ruido por keep-alive timeouts
  // del servidor, a costa de no simular navegadores reales.
  noConnectionReuse: NO_CONNECTION_REUSE,

  scenarios: {
    // Prueba de humo: poca carga, sirve para verificar que todo funciona.
    smoke: {
      executor: 'constant-vus',
      vus: 2,
      duration: '30s',
      exec: 'smoke',
      tags: { scenario: 'smoke' },
    },

    // Carga sostenida: simula tráfico real durante un período.
    load: {
      executor: 'ramping-vus',
      startVUs: 0,
      stages: [
        { duration: RAMP_UP, target: VUS },
        { duration: DURATION, target: VUS },
        { duration: RAMP_DOWN, target: 0 },
      ],
      exec: 'load',
      tags: { scenario: 'load' },
    },
  },

  thresholds: {
    // Menos del 1% de requests deben fallar.
    http_req_failed: ['rate<0.01'],
    // El 95% de las respuestas deben ser menores a 500ms.
    http_req_duration: ['p(95)<500'],
  },
};

export function smoke() {
  hitRandomEndpoint();
  randomSleep();
}

export function load() {
  hitRandomEndpoint();
  randomSleep();
}
