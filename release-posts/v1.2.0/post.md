## moai v1.2.0
### What's new
- `moai daemon install` registers the daemon as a user service that starts at login and launches it immediately
- The service file records the socket directory of the shell running `moai daemon install` on both systemd and launchd, using the invoked moai path
- `install.sh --no-service` and `MOAI_NO_SERVICE=1` bypass service registration during initial installation
- Initial terminal installations via `install.sh` automatically register the daemon as a user service
- `moai update` restarts running services registered by `moai daemon install` for the updated binary, setting `daemon_restarted: true` in `--json`

Release notes: <https://github.com/buzzni/moai/releases/tag/v1.2.0>
Update with `moai update` · install: <https://moai.buzzni.com/guide/#install>
