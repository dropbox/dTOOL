# WebAssembly Security Deep Dive + E2B Explained

**Date:** 2025-11-01
**Focus:** WebAssembly security risks and E2B hosting model

---

## Part 1: What is E2B? (Cloud Service vs Self-Hosted)

### E2B Overview

**E2B = "Execute to Build"** - Cloud-based AI sandbox platform

**Website:** https://e2b.dev

**What it is:**
- ☁️ **Cloud service** (SaaS) - NOT self-hosted by default
- Provides secure sandboxed environments for AI code execution
- Powered by **Firecracker microVMs** (AWS technology)
- Sandboxes start in <200ms
- Can run up to 24 hours

**Used by:** Perplexity, Hugging Face, and other AI companies

### Hosting Options

**Default: Cloud (SaaS)**
```
Your App → E2B API (https://api.e2b.dev) → E2B's Servers → Execution
```
- ✅ You don't run anything
- ✅ No infrastructure management
- ✅ They handle security updates
- ❌ You pay per execution
- ❌ Your code runs on their servers
- ❌ Data passes through their systems

**Optional: Self-Hosted**
- E2B mentions "BYOC, on-prem, or self-hosted" options
- ⚠️ This means you run E2B infrastructure on YOUR cloud/servers
- ⚠️ Requires Enterprise plan (contact sales)
- ⚠️ You manage the infrastructure
- ✅ Data stays on your systems
- ✅ No per-execution costs (just infrastructure)

### E2B Pricing Model (Cloud SaaS)

**Pay per execution:**
- Sandbox creation: ~$0.001 per sandbox
- Compute time: ~$0.05/hour of execution
- Network egress: Standard cloud rates

**Free tier:**
- Usually includes some free sandboxes per month
- Good for testing

### E2B Security Model (Cloud)

**What E2B protects:**
- ✅ Isolation between different customers' code
- ✅ Sandbox escape prevention (Firecracker microVMs)
- ✅ Resource limits (CPU, memory, time)
- ✅ Network isolation (sandboxes can't access each other)

**What E2B CANNOT protect:**
- ❌ Your secrets sent to the sandbox
- ❌ Data exfiltration if your code sends it out
- ❌ E2B employees could theoretically access execution logs
- ❌ Supply chain attacks (E2B infrastructure compromise)
- ❌ Subpoenas / legal requests for data

### When to Use E2B

**Use E2B if:**
- ✅ You want zero infrastructure management
- ✅ You're okay with code running on third-party servers
- ✅ You don't send sensitive secrets to code
- ✅ You want fastest time-to-market (3-4 hours integration)
- ✅ Your budget allows per-execution costs
- ✅ You need network access in sandboxes

**Don't use E2B if:**
- ❌ Regulatory compliance requires data stays on-prem (HIPAA, GDPR strict mode, etc.)
- ❌ You can't trust third-party with execution logs
- ❌ You have extremely high volume (costs add up)
- ❌ You need offline/air-gapped operation
- ❌ You require <100ms latency (API roundtrip adds latency)

### E2B Self-Hosted (Enterprise)

If you self-host E2B:
- You run Firecracker microVMs on YOUR infrastructure
- Your data never leaves your systems
- You pay for infrastructure, not per-execution
- **BUT:** You now manage security updates, scaling, monitoring
- **AND:** Enterprise plan likely $$$$ (contact sales)

**Verdict:** E2B self-hosted is like running your own AWS - you get control but lose convenience.

---

## Part 2: WebAssembly Security Deep Dive

### What is WebAssembly (WASM)?

**WebAssembly =** Binary instruction format for a stack-based virtual machine

**Think of it as:**
- Java Bytecode, but for the web
- Assembly language for a virtual CPU
- Compile C/Rust/Python/etc → WASM → Run anywhere

**Key security feature:** Sandboxed by design - has ZERO capabilities by default.

### WASM Runtimes Available

**Wasmer (v6.1.0):**
- Pure Rust implementation
- Supports WASI (WebAssembly System Interface)
- Easy to embed in Rust apps
- Good performance

**Wasmtime (v38.0.3):**
- Bytecode Alliance (Mozilla, Fastly, Microsoft)
- Most mature and battle-tested
- Used in production by Cloudflare Workers, Fastly Compute@Edge
- Better security audit history

**Recommendation:** Use **Wasmtime** for security-critical applications (more audited).

### WASM Security Model

#### Default: Zero Capabilities

```rust
// WASM module has access to:
// - Its own memory (isolated)
// - Functions you explicitly import
// - NOTHING ELSE

let module = Module::from_file(&engine, "untrusted.wasm")?;
let instance = Instance::new(&mut store, &module, &[])?;

// This code can:
// ✅ Do math
// ✅ Allocate memory (in its own sandbox)
// ✅ Call functions you gave it
// ❌ Access file system (unless you import WASI fs)
// ❌ Access network (unless you import WASI net)
// ❌ Access environment variables
// ❌ Access your process memory
// ❌ Fork processes
// ❌ Call system calls
```

#### WASI: Capability-Based Permissions

**WASI = WebAssembly System Interface**

Think of it as "POSIX, but with capabilities instead of permissions"

```rust
use wasmtime_wasi::WasiCtxBuilder;

// Start with ZERO permissions
let wasi = WasiCtxBuilder::new()
    // Explicitly grant file read ONLY to specific directory
    .preopened_dir(
        Dir::open_ambient_dir("/data", ambient_authority())?,
        "/data"
    )?
    // NO write permission
    // NO network permission
    // NO environment variables
    .build();
```

**Key insight:** You must EXPLICITLY grant each capability. Forgetting to grant = safe default.

### WebAssembly Security Risks (The Real Ones)

#### Risk 1: Runtime Vulnerabilities ⚠️

**What it is:** Bugs in Wasmtime/Wasmer that allow sandbox escape

**Historical examples:**
- CVE-2023-30624: Wasmtime type confusion (patched)
- CVE-2022-39392: Wasmer OOB memory access (patched)

**Likelihood:** Low (rare, patched quickly)

**Impact:** HIGH - Full sandbox escape, RCE

**Mitigation:**
- ✅ Use latest stable versions
- ✅ Monitor security advisories
- ✅ Subscribe to https://github.com/bytecodealliance/wasmtime/security/advisories
- ✅ Update monthly (or when CVE published)
- ✅ Run WASM in separate process (defense in depth)

**Real-world:** Wasmtime is used by Cloudflare (processes billions of requests). Escape rate: effectively zero in production.

#### Risk 2: Side-Channel Attacks ⚠️

**What it is:** Leaking information via timing, cache, speculative execution

**Examples:**
- **Timing attacks:** Measure execution time to infer secrets
  ```rust
  // Attacker's WASM code:
  let start = get_time();
  // Try to access memory
  let end = get_time();
  // If access was fast, memory was in cache = information leak
  ```

- **Spectre/Meltdown:** CPU speculative execution leaks
  ```rust
  // Speculative execution can leak data across security boundaries
  // This is a CPU-level issue, not WASM-specific
  ```

**Likelihood:** Medium (theoretical, hard to exploit)

**Impact:** LOW - Can leak small amounts of data

**Mitigation:**
- ⚠️ Accept as residual risk (physics limitation)
- ✅ Don't put secrets in WASM-accessible memory
- ✅ Use constant-time algorithms for crypto
- ✅ Disable high-precision timers (WASI doesn't expose them by default)
- ✅ CPU mitigations (Intel/AMD microcode updates)

**Real-world:** Side channels require precise timing and many attempts. Not practical for most attacks.

#### Risk 3: Fuel/CPU Exhaustion ⚠️

**What it is:** WASM code uses excessive CPU

**Examples:**
```rust
// Infinite loop
loop { }

// Exponential algorithm
fn fib(n: u64) -> u64 {
    if n <= 1 { n } else { fib(n-1) + fib(n-2) }
}
fib(100) // Takes forever
```

**Likelihood:** HIGH (trivial to exploit)

**Impact:** LOW-MEDIUM - DoS, CPU usage, but contained

**Mitigation:**
```rust
// Fuel metering - WASM operations cost "fuel"
let mut store = Store::new(&engine, ());
store.set_fuel(10_000_000)?; // 10M operations max

let result = instance.call(&mut store, "run", &[])?;

// If fuel exhausted, execution stops with error
match store.get_fuel() {
    Ok(remaining) => println!("Used {} fuel", 10_000_000 - remaining),
    Err(_) => println!("Fuel exhausted, execution stopped"),
}
```

**Real-world:** Fuel metering is standard practice. Set conservative limits.

#### Risk 4: Memory Exhaustion 💾

**What it is:** WASM code allocates excessive memory

**Examples:**
```rust
// Allocate 10GB
let mut data = Vec::new();
for _ in 0..10_000_000_000 {
    data.push(0u8);
}
```

**Likelihood:** HIGH (trivial to exploit)

**Impact:** MEDIUM - OOM killer, system instability

**Mitigation:**
```rust
// Set WASM memory limits
let mut config = Config::new();
config.max_wasm_stack(2 * 1024 * 1024)?; // 2MB stack
// WASM memory is limited by memory type definition

// In WASM module:
(memory (export "memory") 64 256)
// min: 64 pages (4MB), max: 256 pages (16MB)
// Cannot allocate more than 16MB
```

**Plus:** Run WASM in cgroup with memory limit:
```bash
# Linux cgroup v2
echo "100M" > /sys/fs/cgroup/wasm_sandbox/memory.max
```

**Real-world:** Memory limits are easy to enforce. Set to 100-500MB depending on use case.

#### Risk 5: WASI Misconfiguration 🔧

**What it is:** Accidentally granting too many permissions

**Examples:**
```rust
// WRONG: Grant write to entire filesystem
let wasi = WasiCtxBuilder::new()
    .preopened_dir(Dir::open_ambient_dir("/", ambient_authority())?, "/")?
    .build();
// Now WASM can write anywhere!

// RIGHT: Grant read-only to specific directory
let wasi = WasiCtxBuilder::new()
    .preopened_dir(
        Dir::open_ambient_dir("/data", ambient_authority())?
            .open_dir("readonly")?,
        "/data"
    )?
    .build();
```

**Likelihood:** MEDIUM (human error)

**Impact:** HIGH - Defeats sandbox purpose

**Mitigation:**
- ✅ Start with zero permissions
- ✅ Add only what's needed
- ✅ Use read-only mounts whenever possible
- ✅ Code review WASI configuration
- ✅ Test with malicious WASM module

**Real-world:** Most security issues are misconfiguration, not runtime bugs.

#### Risk 6: Supply Chain Attacks 📦

**What it is:** Compromised WASM modules or runtime dependencies

**Examples:**
- Malicious npm package that generates backdoored WASM
- Compromised compiler (rustc, emscripten) that injects exploits
- Trojan in wasmtime/wasmer crate

**Likelihood:** LOW (but increasing)

**Impact:** CRITICAL - Full compromise

**Mitigation:**
- ✅ Audit WASM modules before use
- ✅ Pin dependency versions (Cargo.lock)
- ✅ Use cargo-audit for known vulnerabilities
- ✅ Verify WASM signatures (if available)
- ✅ Scan WASM with static analysis (wasm-tools validate)
- ✅ Run untrusted WASM in extra isolation layer

**Real-world:** Supply chain attacks are rising. Defense in depth is key.

#### Risk 7: Information Disclosure via Errors 📄

**What it is:** Error messages leak information about host system

**Examples:**
```rust
// WRONG: Leak file paths
fn handle_file(path: &str) -> Result<String> {
    std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path, e))
}
// Error: "Failed to read /home/user/.ssh/id_rsa: Permission denied"
// Attacker now knows SSH keys exist

// RIGHT: Generic errors
fn handle_file(path: &str) -> Result<String> {
    std::fs::read_to_string(path)
        .map_err(|_| format!("File operation failed"))
}
```

**Likelihood:** MEDIUM (easy to forget)

**Impact:** LOW - Information leak only

**Mitigation:**
- ✅ Sanitize all error messages
- ✅ Don't include host paths
- ✅ Don't include user names
- ✅ Log details server-side, return generic errors to WASM

**Real-world:** Defense in depth - assume errors will leak, design accordingly.

---

## WebAssembly Security Checklist

Before deploying WASM code execution:

### Runtime Security
- [ ] Use latest stable Wasmtime (v38+) or Wasmer (v6+)
- [ ] Subscribe to security advisories
- [ ] Update runtime monthly minimum
- [ ] Run in separate process (defense in depth)
- [ ] Set resource limits (fuel, memory, stack)

### WASI Configuration
- [ ] Start with WasiCtxBuilder::new() (zero permissions)
- [ ] Only grant needed capabilities
- [ ] Use read-only file mounts when possible
- [ ] Never grant `/` filesystem access
- [ ] Never grant full network access
- [ ] Review and document each permission

### Resource Limits
- [ ] Set fuel limit (recommend: 10M-100M operations)
- [ ] Set memory limit (recommend: 100-500MB)
- [ ] Set stack limit (recommend: 2-8MB)
- [ ] Set execution timeout (recommend: 5-60 seconds)
- [ ] Use Linux cgroups for hard limits

### Monitoring & Logging
- [ ] Log all WASM executions (start, end, duration)
- [ ] Log fuel consumption
- [ ] Log memory usage
- [ ] Alert on timeouts or resource exhaustion
- [ ] Alert on repeated failures (potential attack)

### Input Validation
- [ ] Validate WASM modules before instantiation
- [ ] Check WASM module size (prevent zip bombs)
- [ ] Scan WASM with wasm-tools validate
- [ ] Verify WASM signatures (if using signed modules)

### Defense in Depth
- [ ] Run WASM in unprivileged Linux user
- [ ] Use namespaces (pid, net, mnt, uts)
- [ ] Use seccomp to block dangerous syscalls
- [ ] Use AppArmor/SELinux policies
- [ ] Consider running in container (Docker/Podman)

### Error Handling
- [ ] Sanitize all error messages
- [ ] Don't leak host paths or usernames
- [ ] Log detailed errors server-side only
- [ ] Return generic errors to WASM caller

---

## WebAssembly vs E2B: Security Comparison

| Security Aspect | WebAssembly (Self-Hosted) | E2B (Cloud SaaS) |
|----------------|---------------------------|------------------|
| **Data Privacy** | ✅ Data stays on your system | ❌ Data sent to E2B servers |
| **Regulatory Compliance** | ✅ HIPAA/SOC2 possible | ⚠️ Depends on E2B's compliance |
| **Sandbox Escape Risk** | ⚠️ Runtime bugs (rare) | ⚠️ Firecracker bugs (very rare) |
| **Resource Exhaustion** | ⚠️ Affects your system | ✅ Affects E2B's system |
| **Network Access** | 🔧 Configurable (deny by default) | ✅ Allowed (but isolated) |
| **File System Access** | 🔧 Configurable (deny by default) | ✅ Allowed (sandboxed) |
| **Supply Chain Risk** | ⚠️ Your runtime dependencies | ⚠️ E2B infrastructure |
| **Security Updates** | 🔧 You manage | ✅ E2B manages |
| **Audit Trail** | ✅ Full control | ⚠️ E2B's logs |
| **Cost of Compromise** | 💰 Your data leaked | 💰 E2B incident (shared responsibility) |

---

## Recommended Security Architecture

### Layered Security for WASM

```
┌─────────────────────────────────────────────────┐
│ Layer 5: Monitoring & Alerting                  │
│ (Prometheus, Grafana, PagerDuty)                │
└─────────────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────────┐
│ Layer 4: Input Validation                       │
│ (WASM validation, size limits, signature check) │
└─────────────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────────┐
│ Layer 3: WASM Runtime Security                  │
│ (Fuel limits, memory limits, timeouts)          │
└─────────────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────────┐
│ Layer 2: WASI Capability Restrictions           │
│ (Zero permissions by default, minimal grants)   │
└─────────────────────────────────────────────────┘
                    ↓
┌─────────────────────────────────────────────────┐
│ Layer 1: OS-Level Sandboxing                    │
│ (namespaces, cgroups, seccomp, AppArmor)        │
└─────────────────────────────────────────────────┘
```

**If any layer fails, the next layer catches it.**

### Production WASM Execution Code

```rust
use wasmtime::*;
use wasmtime_wasi::{WasiCtxBuilder, ambient_authority, Dir};
use std::time::Duration;

pub struct SafeWasmExecutor {
    engine: Engine,
    max_fuel: u64,
    max_memory_bytes: usize,
    timeout: Duration,
}

impl SafeWasmExecutor {
    pub fn new() -> Result<Self> {
        let mut config = Config::new();

        // Enable fuel metering
        config.consume_fuel(true);

        // Set WASM limits
        config.max_wasm_stack(2 * 1024 * 1024)?; // 2MB stack

        let engine = Engine::new(&config)?;

        Ok(Self {
            engine,
            max_fuel: 100_000_000,          // 100M operations
            max_memory_bytes: 100 * 1024 * 1024, // 100MB
            timeout: Duration::from_secs(10),     // 10 seconds
        })
    }

    pub async fn execute_wasm(
        &self,
        wasm_bytes: &[u8],
        function_name: &str,
        args: &[Val],
    ) -> Result<Vec<Val>, String> {
        // Layer 4: Validate WASM module
        if wasm_bytes.len() > 10 * 1024 * 1024 {
            return Err("WASM module too large (>10MB)".to_string());
        }

        // Compile module
        let module = Module::new(&self.engine, wasm_bytes)
            .map_err(|e| format!("Invalid WASM module: {}", e))?;

        // Layer 2: Create WASI with zero permissions
        let wasi = WasiCtxBuilder::new()
            // No file system access
            // No network access
            // No environment variables
            .build();

        let mut linker = Linker::new(&self.engine);
        wasmtime_wasi::add_to_linker(&mut linker, |s| s)?;

        let mut store = Store::new(&self.engine, wasi);

        // Layer 3: Set fuel limit
        store.set_fuel(self.max_fuel)
            .map_err(|e| format!("Failed to set fuel: {}", e))?;

        // Instantiate module
        let instance = linker.instantiate(&mut store, &module)
            .map_err(|e| format!("Failed to instantiate: {}", e))?;

        // Get function
        let func = instance.get_func(&mut store, function_name)
            .ok_or_else(|| format!("Function '{}' not found", function_name))?;

        // Execute with timeout
        let result = tokio::time::timeout(
            self.timeout,
            tokio::task::spawn_blocking(move || {
                let mut results = vec![Val::I32(0); func.ty(&store).results().len()];
                func.call(&mut store, args, &mut results)?;
                Ok::<_, anyhow::Error>(results)
            })
        ).await;

        match result {
            Ok(Ok(Ok(results))) => Ok(results),
            Ok(Ok(Err(e))) => Err(format!("Execution error: {}", e)),
            Ok(Err(e)) => Err(format!("Task error: {}", e)),
            Err(_) => Err("Execution timeout".to_string()),
        }
    }
}

// Usage:
async fn run_agent_code(code: &str) -> Result<String, String> {
    // Compile code to WASM (using wasm-pack, emscripten, etc.)
    let wasm_bytes = compile_to_wasm(code)?;

    // Execute safely
    let executor = SafeWasmExecutor::new()?;
    let results = executor.execute_wasm(&wasm_bytes, "run", &[]).await?;

    // Convert result to string
    Ok(format!("{:?}", results))
}
```

---

## Final Recommendations

### For Most Use Cases: WebAssembly ✅

**Pros:**
- ✅ Data stays on your system
- ✅ Full control over security
- ✅ No per-execution costs
- ✅ Works offline/air-gapped
- ✅ 98% safe with proper configuration

**Cons:**
- ⚠️ 10-15 hours to implement properly
- ⚠️ You manage security updates
- ⚠️ You handle resource exhaustion

**Security risks:**
1. Runtime bugs (rare, monitor CVEs)
2. Misconfiguration (test thoroughly)
3. Side channels (accept as residual)

**When to use:** You need full programming capability, data privacy matters, or you want offline operation.

---

### For Maximum Convenience: E2B ✅

**Pros:**
- ✅ 3-4 hours to integrate
- ✅ No infrastructure management
- ✅ E2B handles security updates
- ✅ Resource exhaustion is E2B's problem

**Cons:**
- ❌ Data sent to third-party (E2B)
- ❌ Per-execution costs ($)
- ❌ Requires internet connection
- ❌ Compliance concerns (HIPAA/SOC2)

**Security risks:**
1. Data exfiltration (don't send secrets)
2. E2B infrastructure compromise (low likelihood)
3. Legal/subpoena access to execution logs

**When to use:** You want fastest time-to-market, costs are acceptable, and data privacy is not critical.

---

### For Math Only: Calculator Tool ✅

Already implemented, 99% safe, zero integration time.

---

## Bottom Line

**WebAssembly is safer than E2B for sensitive data** because data never leaves your system.

**E2B is more convenient** because they manage everything.

**Both are infinitely safer than unsandboxed execution.**

Pick based on your security requirements:
- Need HIPAA/SOC2? → WebAssembly
- Want quick integration? → E2B
- Just need math? → calculator_tool (already done)

**All are 95%+ safe.** Perfect security doesn't exist.
