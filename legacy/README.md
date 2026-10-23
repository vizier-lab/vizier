# Vizier WebUI

The Vizier web interface provides a modern UI for managing agents, providers, and configuration.

## Tech Stack

- **Framework**: React Router v7
- **React**: 19
- **Language**: TypeScript
- **Styling**: Tailwind CSS v4
- **State**: Zustand
- **Build**: Vite 7
- **Animations**: Motion (framer-motion successor)
- **Charts**: Recharts
- **Syntax Highlighting**: highlight.js
- **Markdown**: MDX Editor

## Development

```sh
# Install dependencies
npm install

# Start dev server (proxied to backend)
npm run dev

# Typecheck
npm run typecheck

# Build for production
npm run build
```

The dev server runs at `http://localhost:5173` and proxies API requests to the Vizier backend.

## Production

The built output goes to `build/client/` and is served by the Vizier axum server at runtime. The build is triggered automatically by `cargo build` via `build.rs` when `node_modules/` exists.

## Project Structure

```
app/
├── routes/          # Page routes (agents, settings, login)
├── components/      # Reusable UI components
├── services/        # API client (vizier.tsx)
├── stores/          # Zustand state stores
└── lib/             # Utilities and helpers
```
