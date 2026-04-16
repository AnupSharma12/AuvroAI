# AuvroAI
AuvroAI is a desktop AI chat app written in Rust with a focus on fast startup, local-first responsiveness, streaming chat, and measurable optimization work.

It uses `eframe/egui` for the UI, Supabase for chat history and profile data, and a provider abstraction that can talk to HackClub AI or OpenRouter. If no remote provider is configured, the app falls back to a local mock provider for demo and development workflows.

## Features

- Desktop chat UI with sidebar session management, a central conversation panel, and settings.
- Streaming assistant responses with cancellation support.
- Secure API key storage through the operating system keychain, with encrypted local fallback.
- Session history persisted in Supabase.
- Lazy-loaded model metadata so the settings panel does not slow down startup.
- Response caching and memory-focused streaming optimizations.
- Cross-platform release configuration for Windows, Linux, and macOS builds.

## Requirements

- Rust toolchain from <https://rustup.rs>
- Supabase project with the `profiles` table and auth/storage setup described in [src/api/profile.rs](src/api/profile.rs)
- One of the supported provider configurations:
	- HackClub AI
	- OpenRouter

## Setup

1. Copy [.env.example](.env.example) to `.env`.
2. Fill in the required values for Supabase and at least one provider.
3. Run the app with `cargo run`.

### Environment Variables

| Variable | Purpose |
|---|---|
| `SUPABASE_URL` | Supabase project URL. |
| `SUPABASE_PUBLISHABLE_KEY` | Supabase anon/publishable key used by the client. |
| `AUVRO_API_KEY` | HackClub AI key. |
| `AUVRO_ENDPOINT` | HackClub AI endpoint. |
| `AUVRO_MODEL` | HackClub AI model name. |
| `OPENROUTER_API_KEY` | Optional OpenRouter key. |
| `OPENROUTER_BASE_URL` | Optional OpenRouter API base URL. |
| `OPENROUTER_MODEL` | Optional OpenRouter model name. |
| `AUVRO_CACHE_TTL_SECS` | Response cache TTL. |
| `AUVRO_CACHE_MAX_MB` | Response cache size cap. |

## Usage

- Start a new chat from the sidebar.
- Type a message and press Enter to send.
- Use Ctrl+Enter for a newline in the composer.
- Open Settings to manage display name, email, password, theme, and model metadata.
- Copy assistant messages with the copy button in each response bubble.

## Installer Builds

This repository includes native installer packaging for each platform:

- Windows: `.msi` via `cargo-wix`
- Linux: `.deb` via `cargo-deb`
- macOS: `.dmg` via `cargo-bundle` + `create-dmg`

### Local Commands

- Windows (PowerShell):
	- `cargo install cargo-wix --locked`
	- `cargo wix --release --nocapture`
- Linux:
	- `cargo install cargo-deb --locked`
	- `cargo deb`
- macOS:
	- `cargo install cargo-bundle --locked`
	- `brew install create-dmg`
	- `cargo bundle --release`
	- `create-dmg --volname "AuvroAI" dist/AuvroAI.dmg target/release/bundle/osx/`

### CI Build

Run the GitHub Actions workflow at `.github/workflows/build-installers.yml` using `workflow_dispatch`, or push a tag like `v0.1.0`.
The workflow uploads `windows-msi`, `linux-deb`, and `macos-dmg` artifacts.

## Implementation Notes

- [src/provider.rs](src/provider.rs) defines the provider trait and failover routing between HackClub, OpenRouter, and the local mock provider.
- [src/chat_pipeline.rs](src/chat_pipeline.rs) builds the chat payload, handles streaming SSE/chunked responses, and feeds the response cache.
- [src/main.rs](src/main.rs) owns session state, streaming orchestration, and conversation history.
- [src/ui/chat.rs](src/ui/chat.rs) renders the message feed and live streaming state.
- [src/ui/settings.rs](src/ui/settings.rs) triggers lazy loading for model metadata.
- [src/api/profile.rs](src/api/profile.rs) handles Supabase profile, avatar, and auth-related REST calls.

## Optimization

This pass focused on reducing startup cost, cutting allocation churn in the chat pipeline, trimming the dependency graph, and shrinking the release binary without changing the app's core behavior.

### What Changed

- Technique 1 added an application-level response cache so repeated prompts return immediately instead of recomputing a response.
- Technique 2 moved session history and model metadata loading behind user actions, which reduced cold-start work.
- Technique 3 reduced allocation pressure in the streaming path by reusing buffers and switching immutable chat payload data to shared `Arc<str>` values.
- Technique 4 removed unused dependencies and disabled unnecessary default features to simplify the build graph.
- Technique 5 tightened the release profile with thin LTO, single codegen units, symbol stripping, and `panic = "abort"`.

### Implementation Notes

- [src/chat_pipeline.rs](src/chat_pipeline.rs) uses `Arc<str>` in `ApiMessage`, reuses the SSE line buffer, and avoids per-line owned string copies where possible.
- [src/main.rs](src/main.rs) returns shared conversation lines and appends streaming chunks instead of cloning the whole buffer on every update.
- [src/provider.rs](src/provider.rs) threads `Arc<str>` conversation history through the provider trait and implementations.
- [Cargo.toml](Cargo.toml) removes unused direct dependencies, disables unneeded default features, and adds the release profile settings.

### Before / After Results

| Metric | Before | After |
|-------|--------|-------|
| Cold start time | 1130 ms | 0 ms |
| Avg response latency P50 / P95 | 220 ms / 225 ms | 0 ms cached / not separately re-benchmarked |
| Peak RSS memory | 22088.00 MB | 16304.00 MB |
| Release binary size | 30.47 MB | 19.91 MB |

### Benchmark Setup

- CPU: AMD EPYC 7763 64-Core Processor
- OS: Windows 11 (26100)
- Rust toolchain: rustc 1.94.1 (e408947bf 2026-03-25)
- Date: 2026-04-15

### Commands

- `cargo run --bin technique3_memory_benchmark`
- `cargo +nightly udeps --all-targets`
- `cargo build --release`

## Troubleshooting

- If the app panics with a missing `wgpu` backend, make sure you are running a build with the platform-specific backend features enabled in [Cargo.toml](Cargo.toml).
- If sign-in or profile calls fail, verify the Supabase URL, publishable key, and database setup.
- If the app starts in mock/demo mode, check that a provider API key and endpoint are configured.

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for the full text.
