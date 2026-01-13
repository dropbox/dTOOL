# Buildkite Agent Provisioning (dterm-core)

**Last Updated:** 2025-12-31 (Iteration 457)

## Purpose
Unblock CI while GitHub Actions hosted runners are disabled by provisioning
Buildkite agents for the queues defined in `.buildkite/pipeline.yml`.

## Prereqs
- Buildkite org and pipeline with access to this repo
- Agent tokens for each host
- Hosts for queues: macos, windows, linux (verify can share linux)
- Rust toolchains installed (stable everywhere, nightly + miri on verify)

## Step-by-step
1. Create pipeline
   - Connect the repository in the Buildkite UI
   - Configure the pipeline to read `.buildkite/pipeline.yml` from the repo
2. Confirm queue names
   - Defaults are set in `.buildkite/pipeline.yml`:
     - macos, windows, linux, verify (verify defaults to linux queue)
   - Override with env vars if needed:
     - DTERM_CI_MACOS_QUEUE, DTERM_CI_WINDOWS_QUEUE, DTERM_CI_LINUX_QUEUE, DTERM_CI_VERIFY_QUEUE
3. Install Buildkite agent on each host
   - Follow the official Buildkite install docs for the OS
   - Ensure `buildkite-agent` is on PATH
4. Configure agent token + queue tags
   - Example config snippet:
     ```
     token="BUILDKITE_AGENT_TOKEN"
     tags="queue=macos"
     ```
   - Config locations (common defaults):
     - macOS (Homebrew): `/opt/homebrew/etc/buildkite-agent/buildkite-agent.cfg` or `/usr/local/etc/buildkite-agent/buildkite-agent.cfg`
     - Linux (deb/rpm): `/etc/buildkite-agent/buildkite-agent.cfg`
     - Windows (installer): `C:\buildkite-agent\buildkite-agent.cfg`
5. Start the agent
   - Preferred: start via service manager for the OS
   - Quick validation: `buildkite-agent start` (foreground)
6. Install toolchains
   - macos/windows/linux queues: Rust stable + platform toolchain
   - verify queue: Rust nightly + `miri` component
   - Linux also needs ALSA headers: `libasound2-dev`
7. Verify
   - Agents appear online in Buildkite UI with correct queue tags
   - Trigger pipeline and confirm each step is assigned to the intended queue
   - Record agent status and results in `docs/PENDING_WORK.md`

## Queue mapping
- `macos`: macOS host with Xcode/Command Line Tools + Rust stable
- `windows`: Windows host with Visual Studio Build Tools + Windows SDK + Rust stable
- `linux`: Linux host with GCC/Clang + Rust stable + ALSA headers
- `verify`: Linux host with Rust nightly + MIRI (can share `linux` queue)

## Troubleshooting
- If a step is stuck, verify the queue name matches agent tags and pipeline env.
- If verify fails, confirm `rustup component add miri --toolchain nightly` and
  `cargo +nightly miri test` works on the verify agent.
