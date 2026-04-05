# Changelog

## 0.4.1 (2026-04-05)

### Bug Fixes

- improve session parsing performance and unicode-safe truncation (#6) ([343ea88](https://github.com/urmzd/agentspec/commit/343ea88fd18836e9909d074afc8f9e45d4a95e15))

### Miscellaneous

- add linguist overrides to fix language stats (#7) ([1f49ea1](https://github.com/urmzd/agentspec/commit/1f49ea19e0446e7c20e36052847460dc5523e8de))

[Full Changelog](https://github.com/urmzd/agentspec/compare/v0.4.0...v0.4.1)


## 0.4.0 (2026-04-05)

### Features

- unify all resource types under tagged entity model ([85797e8](https://github.com/urmzd/agentspec/commit/85797e854619232832e021f09101db8fd9afd2ba))

### Refactoring

- improve code formatting and consistency ([e8fd9ef](https://github.com/urmzd/agentspec/commit/e8fd9efb65a597413961f727a34c7a26e747cfca))
- remove legacy lockfile consumers and dead code ([a40cab8](https://github.com/urmzd/agentspec/commit/a40cab86e495ce051c5895a485d98827a49d17cd))

### Miscellaneous

- merge remove-dead-code branch into unified entity model ([6a14e7c](https://github.com/urmzd/agentspec/commit/6a14e7cacf8c49816333bf5b3d2b244155330569))
- **github**: update verification command in CI workflow ([72a832b](https://github.com/urmzd/agentspec/commit/72a832b1e95fd01e711c6a7b9d462f8dfce1080e))
- **hooks**: add git commit message hook infrastructure ([4e03756](https://github.com/urmzd/agentspec/commit/4e03756cbf78f538ae5d60265621a1803463cba2))

[Full Changelog](https://github.com/urmzd/agentspec/compare/v0.3.0...v0.4.0)


## 0.3.0 (2026-04-05)

### Features

- **settings**: add tool settings abstraction ([2e13ed3](https://github.com/urmzd/agentspec/commit/2e13ed382d3906463144f03167fc3eebb8a5e406))
- **session**: add universal session discovery ([46e7146](https://github.com/urmzd/agentspec/commit/46e71461366fb973032fa1f409778e69ab74d701))
- **session**: implement Claude and Codex session adapters ([ae7d4d2](https://github.com/urmzd/agentspec/commit/ae7d4d2ef4b16904e8ddb5fc8cc6bfbd4c21f663))
- **session**: define session IR and adapter trait ([6abea49](https://github.com/urmzd/agentspec/commit/6abea490d51853f0328593ab501b393b38a919fe))

### Bug Fixes

- resolve clippy and fmt CI failures ([64ca63b](https://github.com/urmzd/agentspec/commit/64ca63be8459203464947d0d370db3b8af393000))
- make install.sh executable ([22f92a6](https://github.com/urmzd/agentspec/commit/22f92a64b4adeb26dda08e72673b55689814d54b))

### Documentation

- add architecture plan and standards reference ([f0794a8](https://github.com/urmzd/agentspec/commit/f0794a84e4ee3cb49d41504d16b4f3f86466092d))

### Miscellaneous

- remove dead code and incorrect allow(dead_code) annotations ([55272fd](https://github.com/urmzd/agentspec/commit/55272fd876c6bd70b80723218edc0330f54411e2))
- configure git commit message hook ([46cf65e](https://github.com/urmzd/agentspec/commit/46cf65e3da6a739e6faf246463e4066920079893))
- bump version to 0.2.0 ([b098b0d](https://github.com/urmzd/agentspec/commit/b098b0d5baec616b5c64d2d91e8aaa7b2792feec))

[Full Changelog](https://github.com/urmzd/agentspec/compare/v0.2.0...v0.3.0)


## 0.2.0 (2026-04-04)

### Features

- add inventory management with discover, manage, verify, dedup, and memory commands ([f4295f1](https://github.com/urmzd/agentspec/commit/f4295f128e8a8054d7274202109295b6f1c6bb1e))
- add session management and update README ([d9c49ec](https://github.com/urmzd/agentspec/commit/d9c49ec48b0d41a0cda54dc3b9ea85eb237758da))

### Documentation

- remove legacy lock file reference from README ([a100070](https://github.com/urmzd/agentspec/commit/a100070e40c4da1c70aea25ab560646e03a59608))

[Full Changelog](https://github.com/urmzd/agentspec/compare/v0.1.0...v0.2.0)


## 0.1.0 (2026-04-04)

### Features

- initial release of agentctl ([2b30183](https://github.com/urmzd/agentspec/commit/2b30183d8649838b604408b9c28926037a3ef07d))

### Bug Fixes

- switch reqwest to rustls-tls for musl builds ([f5e799d](https://github.com/urmzd/agentspec/commit/f5e799d3134c349604714a8081480c07d2012b64))

### Refactoring

- rename to agentspec ([006a27a](https://github.com/urmzd/agentspec/commit/006a27a2fe023f9754088b7ade3d0539a3777e8f))

### Miscellaneous

- fix rustfmt formatting ([125ec42](https://github.com/urmzd/agentspec/commit/125ec4252190bc0a06b8f8455405ff937f9a2d66))
