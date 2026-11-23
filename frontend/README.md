# Sphinx frontend

A responsive login, sign-up, and personal-results frontend for Sphinx.

## Run locally

Start PostgreSQL and the Rust API from the repository root:

```bash
cp -n .env.example .env
docker compose up -d --wait db
cargo run
```

Then serve the static frontend on port 3000:

```bash
python3 -m http.server 3000 --directory frontend/dist
```

Open `http://127.0.0.1:3000`. The frontend calls the API at
`http://127.0.0.1:8000` by default.

To use another API deployment without rebuilding the static files, add an
`apiBase` query parameter:

```text
http://127.0.0.1:3000/?apiBase=https://api.example.com
```

## Connected API flows

- `POST /api/v1/auth/register`
- `POST /api/v1/auth/login`
- `POST /api/v1/auth/logout`
- `GET /api/v1/quizzes/{quiz_id}/my-results`

The results request uses the logged-in user's Bearer token and renders the
latest summary, attempt history, and per-revision rankings. "Remember me"
stores the session in local storage; otherwise it is kept in session storage.
