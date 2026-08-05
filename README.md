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
hi