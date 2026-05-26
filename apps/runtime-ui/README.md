# Runtime UI

Svelte + Vite based monitoring UI for the local Preview Runtime.

## Development

```sh
npm install
npm run dev -- --host 127.0.0.1 --port 5173
```

The Vite dev server proxies `/api/*` to `http://127.0.0.1:18090` by default. Override it with:

```sh
VITE_RUNTIME_API_TARGET=http://127.0.0.1:18090 npm run dev
```

## Verification

```sh
npm run check
npm run build
```
