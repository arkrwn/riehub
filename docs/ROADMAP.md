# Delivery roadmap

The project will ship usable product breadth before broad validation.
The existing `v0.0.1` tag is a historical foundation baseline, not a
complete or production-ready product. The `0.0.1` feature-build phase
continues on the development branch without moving that tag.

| Phase | Version | Goal | Exit criteria |
|---|---|---|---|
| Build the product | 0.0.1 | Integrate the planned user-facing features across identity, the router, providers, MCP, automation, governance, web UI, and deployment. | The complete product builds as one system and is pushed for review. Use focused checks while building. Record known gaps. |
| Verify and harden | 0.0.2 | Run end-to-end journeys, security checks, cross-platform CI, and operational tests. Fix the gaps found in 0.0.1. | Required tests pass in CI on Linux and macOS. Critical defects and release-blocking security issues are fixed. Upgrade, backup, and restore paths are proven. |
| Production release | 0.1.0 | Ship a self-hosted product that operators can deploy and maintain in production. | Setup, login, routing, recovery, and administration journeys pass end to end. Deployment, TLS, backup, restore, upgrades, security, and support documentation are ready. |
| Compatibility maturity | 1.0.0 | Keep the product stable for long-term use. | Public interfaces have compatibility rules, release artifacts are verifiable, and the production system has completed a sustained release soak. |

The feature list in the subsystem specifications describes the intended
product. It does not mean those features are already implemented. The
[progress page](PROGRESS.md) records the current repository state.
