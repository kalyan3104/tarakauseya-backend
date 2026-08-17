# Tara Kauseya API

Production-style Rust/Axum backend for the Tara Kauseya storefront. It follows the supplied Merchmix layout: routes, handlers, models, middleware, database migrations, configuration, uploads and tests are separated instead of living in one server file.

## Run locally

```bash
cd Backend
cp .env.example .env # set a real JWT_SECRET before production
cargo run
```

The server runs at `http://127.0.0.1:3000`, stores its SQLite database at `Backend/tara-kauseya.db`, imports the existing top-level `data/*.json` files on first run, and saves uploads in `Backend/uploads/`.

The frontend should be started separately with `npm run dev` from `Frontend`. Its Vite proxy forwards `/api` and `/uploads` to port 3000.

Authentication uses Argon2 password hashing and signed JWTs. Development verification uses code `000000`; configure transactional email delivery before public deployment.

## Deploy on Render

This repository includes `render.yaml`. Create (or update) the Render web
service with these settings:

- **Build command:** `cargo build --release --locked`
- **Start command:** `./target/release/tara-kauseya-api`
- **Health check path:** `/api/health`

Set `DATABASE_URL`, `DIRECT_URL`, and `FRONTEND_ORIGIN` in Render's Environment
page. `JWT_SECRET` must be a long random value. Render provides `PORT`; the app
listens on `0.0.0.0` by default so its proxy can reach it.

### Product image storage

The Render filesystem is temporary, so it must not be used for product images
in production. Create a **public** Supabase Storage bucket named
`product-images`, then set these Render environment variables from the same
Supabase project:

- `SUPABASE_URL`
- `SUPABASE_SERVICE_ROLE_KEY`
- `SUPABASE_STORAGE_BUCKET=product-images`

Production uploads are limited to 20 MB each. The API refuses uploads when
durable storage is not configured, rather than returning an image URL that will
break after a Render restart or deploy.
hi
