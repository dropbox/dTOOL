# CI Alternatives for dterm

Purpose: unblock Windows/Linux CI and mic E2E tests while GitHub Actions hosted
runners are disabled.

## Decision (Iteration 389)
Primary CI path is Buildkite until GitHub Actions hosted runners are enabled.
Next steps: provision macOS/Windows/Linux/verify Buildkite agents, run
`.buildkite/pipeline.yml`, and record results in `docs/PENDING_WORK.md`.

## Requirements
- Windows and Linux runners with native toolchains (MSVC + Windows SDK, GCC/Clang)
- Rust toolchain + nightly (MIRI) since the verify job fails on MIRI errors
- Ability to run ignored tests on Windows and PTY integration tests on Linux
- Access to repo secrets if needed (none required for current test matrix)

## Option A: GitHub Actions self-hosted runners (RECOMMENDED)

This is the recommended path since the workflow is already configured.

### Step-by-step setup

#### 1. Create runner group (GitHub Enterprise admin)
1. Go to org settings → Actions → Runner groups
2. Create group `dterm-ci`, restrict to `dropbox/dTOOL/dterm` repo only

#### 2. Provision hosts
- **Linux**: Ubuntu 22.04+ VM or bare metal (4+ cores, 8GB+ RAM)
- **Windows**: Windows 11/Server 2022 VM (4+ cores, 8GB+ RAM)
- **macOS**: Optional (hosted runners might work; check Enterprise policy)

#### 3. Install runner on Linux host
```bash
# Download runner (get URL from repo Settings → Actions → Runners → New self-hosted runner)
mkdir actions-runner && cd actions-runner
curl -o actions-runner-linux-x64-2.321.0.tar.gz -L https://github.com/actions/runner/releases/download/v2.321.0/actions-runner-linux-x64-2.321.0.tar.gz
tar xzf ./actions-runner-linux-x64-2.321.0.tar.gz

# Configure with labels
./config.sh --url https://github.com/dropbox/dTOOL/dterm --token <TOKEN> --labels self-hosted,linux,dterm-ci

# Install as service
sudo ./svc.sh install
sudo ./svc.sh start
```

#### 4. Install toolchain on Linux
```bash
sudo apt-get update && sudo apt-get install -y build-essential clang pkg-config llvm libasound2-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source ~/.cargo/env
rustup toolchain install nightly
rustup component add miri --toolchain nightly
```

#### 5. Install runner on Windows host
```powershell
# Download runner (get URL from repo Settings → Actions → Runners)
mkdir actions-runner; cd actions-runner
Invoke-WebRequest -Uri https://github.com/actions/runner/releases/download/v2.321.0/actions-runner-win-x64-2.321.0.zip -OutFile actions-runner-win-x64-2.321.0.zip
Expand-Archive -Path actions-runner-win-x64-2.321.0.zip -DestinationPath .

# Configure with labels
.\config.cmd --url https://github.com/dropbox/dTOOL/dterm --token <TOKEN> --labels self-hosted,windows,dterm-ci

# Install as service (run as Administrator)
.\svc.cmd install
.\svc.cmd start
```

#### 6. Install toolchain on Windows
1. Install Visual Studio Build Tools 2022 with C++ workload and Windows 11 SDK
2. Install Rust: `winget install Rustlang.Rustup` or download from rustup.rs
3. Add nightly: `rustup toolchain install nightly`

#### 7. Configure repo variables (optional override)
If you want to customize runner labels, set these repo variables:
- `DTERM_LINUX_RUNNER`: `["self-hosted","linux","dterm-ci"]`
- `DTERM_WINDOWS_RUNNER`: `["self-hosted","windows","dterm-ci"]`
- `DTERM_MACOS_RUNNER`: `["self-hosted","macOS","dterm-ci"]` or `"macos-latest"`
- `DTERM_VERIFY_RUNNER`: `["self-hosted","linux","dterm-ci"]`

### Verification
After runners are online:
1. Push a commit or open a PR to trigger CI
2. Check Actions tab for job pickup
3. All 4 jobs (macos, windows, linux, verify) should run

### Runner readiness checklist (record in `docs/PENDING_WORK.md`)
- Runner shows **Idle** with expected labels in GitHub → Settings → Actions → Runners
- Service reports healthy:
  - Linux: `sudo ./svc.sh status`
  - Windows (Admin): `.\svc.cmd status`
- Toolchain present:
  - `rustc -Vv`
  - `cargo -V`
  - `rustup toolchain list`
  - `rustup component list --installed | rg miri`
- MIRI runs cleanly: `cargo +nightly miri test -p dterm-core parser:: -- --test-threads=1`

### Security notes
- Prefer ephemeral or auto-reimaged runners to avoid state drift
- Restrict runner group to this repo only
- Treat runner tokens as secrets; rotate periodically

### Workflow label example
```yaml
jobs:
  linux:
    runs-on: [self-hosted, linux, dterm-ci]
  windows:
    runs-on: [self-hosted, windows, dterm-ci]
```

### Repo workflow defaults
- `.github/workflows/ci.yml` defaults to self-hosted labels via repo variables:
  - `DTERM_MACOS_RUNNER`, `DTERM_WINDOWS_RUNNER`, `DTERM_LINUX_RUNNER`, `DTERM_VERIFY_RUNNER`
- Each variable should be a JSON array or string accepted by `runs-on`.
  - Example (self-hosted): `["self-hosted","macOS","dterm-ci"]`
  - Example (hosted fallback): `"macos-latest"`

## Option B: Alternative CI providers
- Buildkite: self-hosted agents; pipeline in `.buildkite/pipeline.yml`.
- CircleCI: self-hosted runners; config in `.circleci/config.yml`.
- GitLab CI or Azure DevOps: mirror repo and configure Windows/Linux runners.

### Buildkite pipeline (included)
The repo now includes a ready-to-run Buildkite pipeline:
- Config: `.buildkite/pipeline.yml`
- Scripts: `.buildkite/scripts/macos.sh`, `.buildkite/scripts/linux.sh`,
  `.buildkite/scripts/windows.ps1`, `.buildkite/scripts/verify.sh`
 - Step-by-step provisioning: `docs/BUILDKITE_PROVISIONING.md`

**Agent requirements:**
- Rustup with stable toolchain on macOS/Linux/Windows
- Nightly toolchain + MIRI component on the verify agent
- Linux: ALSA headers (`libasound2-dev`)
- Windows: Visual Studio Build Tools + Windows SDK

**Queue labels (override via Buildkite env vars if desired):**
- `DTERM_CI_MACOS_QUEUE` (default `macos`)
- `DTERM_CI_WINDOWS_QUEUE` (default `windows`)
- `DTERM_CI_LINUX_QUEUE` (default `linux`)
- `DTERM_CI_VERIFY_QUEUE` (default `linux`)

### Decision notes
- Buildkite is the lowest friction if you already have agents; it mirrors GitHub Actions capabilities.
- CircleCI requires org-level setup but keeps configs closer to GitHub Actions format.
- GitLab/Azure require repo mirroring and extra auth plumbing.

## Option C: Temporary manual gate
- Run `cargo test`, `cargo clippy --all-features`, `cargo +nightly miri test`,
  and `cargo kani` on Windows/Linux manually.
- Capture outputs and record results in `docs/PENDING_WORK.md` until CI is ready.

### When to use Option C
- Only as a short-lived stopgap while provisioning runners.
- Schedule a weekly cadence to avoid stale Windows/Linux signal.

## Recommendation checklist
- Decide on primary path (enable hosted runners vs self-hosted vs alternate CI).
- If self-hosted: pick host locations, create runner group, set labels, update CI.
- If alternate CI: choose provider, add config file, and document in `docs/BUILDING.md`.
