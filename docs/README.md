# Angzarr Documentation

This documentation site is built using [Docusaurus](https://docusaurus.io/).

## Local Development

```bash
cd docs
npm install
npm start
```

This starts a local development server at `http://localhost:3000`. Most changes are reflected live without restarting.

## Build

```bash
npm run build
```

Generates static content into `build/` for deployment.

## Deployment

Documentation is automatically deployed to GitHub Pages via GitHub Actions when changes are pushed to `main`. See `.github/workflows/deploy-docs.yml`.
