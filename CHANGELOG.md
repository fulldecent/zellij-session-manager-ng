# Changelog

## [0.1.0](https://github.com/fulldecent/zellij-session-manager-ng/compare/v0.0.0...v0.1.0) (2026-11-25)


### Features

* show sessions in a theme-colored table ([c516e75](https://github.com/fulldecent/zellij-session-manager-ng/commit/c516e75869d0ea0215860abe03ab193483ff3320))

## Changelog

## Unreleased

- Find lists running sessions and saved sessions.
- ENTER attaches to a running session, resurrects a saved session, or opens a layout list for a new name.
- Session updates use the event payload. A key does not request a fresh session dump.
- Rename, kill, disconnect, save, and kill-all use the same commands as the built-in session manager.
