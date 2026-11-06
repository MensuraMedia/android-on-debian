# Contributing

1. Read `docs/architecture.md` (module contract, config precedence, templating).
2. Keep changes modular: a new capability is a new `modules/<name>/`, a new use
   case is a new `profiles/<name>.profile`, a new app type is a new `templates/<name>/`.
3. Versions and paths go in `config/defaults.env` — never inline in modules.
4. Before opening a PR: `bash -n` on changed scripts, `./adx list`, and for
   template changes scaffold an app and run `./gradlew recordRoborazziDebug`.
5. Add an entry under `[Unreleased]` in `CHANGELOG.md`.
