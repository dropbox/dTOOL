# Security Advisories Tracking

Status as of: 2025-12-18

## Active Vulnerabilities (4)

### 1. ring 0.16.20 - RUSTSEC-2025-0009
- **Severity**: Not specified
- **Title**: Some AES functions may panic when overflow checking is enabled
- **Date**: 2025-03-06
- **Solution**: Upgrade to >=0.17.12
- **Blocked by**: milvus-sdk-rust 0.1.0 → tonic 0.8.3 → tokio-rustls 0.23.4 → rustls 0.20.9 → ring 0.16.20
- **Status**: BLOCKED - milvus-sdk-rust is unmaintained/not updated
- **Action**: Consider replacing dashflow-milvus with alternative or waiting for upstream update

### 2. rsa 0.9.8 - RUSTSEC-2023-0071
- **Severity**: 5.9 (medium)
- **Title**: Marvin Attack: potential key recovery through timing sidechannels
- **Date**: 2023-11-22
- **Solution**: No fixed upgrade available!
- **Affected paths**:
  - sqlx-mysql 0.8.6 → rsa 0.9.8
  - opendal 0.54.1 → reqsign 0.16.5 → rsa 0.9.8
- **Status**: BLOCKED - No upstream fix available yet
- **Action**: Monitor rsa crate for security updates

### 3. rustls 0.20.9 - RUSTSEC-2024-0336
- **Severity**: 7.5 (high)
- **Title**: `rustls::ConnectionCommon::complete_io` could fall into an infinite loop based on network input
- **Date**: 2024-04-19
- **Solution**: Upgrade to >=0.23.5 OR >=0.22.4, <0.23.0 OR >=0.21.11, <0.22.0
- **Blocked by**: milvus-sdk-rust 0.1.0 → tonic 0.8.3 → tokio-rustls 0.23.4 → rustls 0.20.9
- **Status**: BLOCKED - Same issue as ring, milvus-sdk-rust uses old tonic
- **Action**: Same as #1

### 4. time 0.1.45 - RUSTSEC-2020-0071
- **Severity**: 6.2 (medium)
- **Title**: Potential segfault in the time crate
- **Date**: 2020-11-18
- **Solution**: Upgrade to >=0.2.23
- **Blocked by**: playwright 0.0.20 → zip 0.5.13 → time 0.1.45
- **Status**: BLOCKED - playwright is on latest version (0.0.20), old time is their transitive dep
- **Action**: File issue with playwright-rust project or wait for upstream update

## Unmaintained Warnings (12)

Notable unmaintained crates we depend on:

1. **backoff 0.4.0** (RUSTSEC-2025-0012) - Used by async-openai and neo4rs
2. **fxhash 0.2.1** (RUSTSEC-2025-0057) - Recommend replacing with rustc-hash
3. **google-apis-common 7.0.0** - Used by google_youtube3
4. **instant 0.1.13** (RUSTSEC-2024-0384) - Recommend web-time as alternative
5. **paste 1.0.15** (RUSTSEC-2024-0436) - Proc macro crate

## Mitigation Status

### Cannot Fix (Blocked by third-party crates)
- ✗ ring 0.16.20 - Blocked by milvus-sdk-rust
- ✗ rustls 0.20.9 - Blocked by milvus-sdk-rust
- ✗ time 0.1.45 - Blocked by playwright
- ✗ rsa 0.9.8 - No fix available

### Can Fix (Requires code changes)
- None currently

### Monitoring
- All vulnerabilities are transitive dependencies
- Cannot be fixed without upstream updates
- Consider opening issues with upstream crate maintainers:
  - milvus-sdk-rust (for ring/rustls)
  - playwright-rust (for time)
  - rsa crate maintainers (no fix available)

## Recommendations

1. **Short term**: Document these as known issues in README
2. **Medium term**:
   - Consider replacing dashflow-milvus with alternative vector store if security is critical
   - Replace backoff 0.4.0 with maintained alternative (backon or backoff 0.3.x with updates)
3. **Long term**:
   - Monitor upstream crates for updates
   - Consider contributing patches to upstream crates if possible

## Next Review
Check security advisories again at #1200 or when major dependency updates occur.
Last reviewed at #1128 (2025-12-18) - no changes from prior review.
