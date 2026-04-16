# Changelog

## [1.0.0] - Unreleased

## [0.10.0] - 2026-04-17

Targets behavioral parity with upstream [tofi `0.9.1`](https://github.com/philj56/tofi).

### Known differences from upstream

- Single binary only: use `tofi --drun` / `tofi --run` instead of `tofi-drun` / `tofi-run` symlinks.
- `tofi-run` uses direct PATH scanning rather than `compgen`. Results are equivalent.
- Pixel-identical rendering is not guaranteed across hardware/drivers (documented non-goal).
