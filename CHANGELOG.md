# Changelog

## 0.7.2 (2026-04-12)

### Refactoring

- remove agentspec-provider crate ([b13ed54](https://github.com/urmzd/agentspec/commit/b13ed545ef82d19a7659ed88b8dfa9fda8824771))

### Miscellaneous

- **release**: clean up workflow and drop agentspec-provider publish ([1d241a5](https://github.com/urmzd/agentspec/commit/1d241a591a6c55f3a1f39616cd14aee6637c5d8e))

[Full Changelog](https://github.com/urmzd/agentspec/compare/v0.7.1...v0.7.2)


## 0.7.1 (2026-04-09)

### Bug Fixes

- **ci**: remove --allow-dirty from cargo publish ([22beaf9](https://github.com/urmzd/agentspec/commit/22beaf9b9737e244fe7181f622bb7dfb5fae7228))

[Full Changelog](https://github.com/urmzd/agentspec/compare/v0.7.0...v0.7.1)


## 0.7.0 (2026-04-09)

### Features

- **cli**: add agentspec-update dep and install.sh ([84a943c](https://github.com/urmzd/agentspec/commit/84a943c2a9e4618ca8ade81da7e1ca40feef7642))

### Bug Fixes

- **ci**: use --format json instead of --json in integration test ([d8bf307](https://github.com/urmzd/agentspec/commit/d8bf3071d676249bb6ac5473e81817e878f9c711))

### Refactoring

- **cli**: rename self-update to update, add version command ([83efa21](https://github.com/urmzd/agentspec/commit/83efa2119ea65359876baf474b20676ded2a48eb))

### Miscellaneous

- **cli**: reformat long function calls in main.rs ([cce15c0](https://github.com/urmzd/agentspec/commit/cce15c0bf943f21eb4c3169d6dd057ea767a2694))

[Full Changelog](https://github.com/urmzd/agentspec/compare/v0.6.0...v0.7.0)


## 0.6.0 (2026-04-09)

### Features

- implement library crates for SDK and tooling ([d393859](https://github.com/urmzd/agentspec/commit/d3938594ab68aad717deafea92517210dd2b1582))

### Documentation

- update README and ROADMAP for v0.5.0 ([8ad9539](https://github.com/urmzd/agentspec/commit/8ad95392ffc3ae90b4fc3c357fd9b6e375bac6c5))

### Refactoring

- **link**: use guard clauses with && for condition chaining ([b5a2c38](https://github.com/urmzd/agentspec/commit/b5a2c38c4ff141889c880a5aaef85d4911fa08a7))
- **provider**: use #[derive(Default)] for config structs ([1e88d10](https://github.com/urmzd/agentspec/commit/1e88d104d6e6414014836095ad6dd15717121a2b))
- remove old source files from root after relocation to crates ([677d1d6](https://github.com/urmzd/agentspec/commit/677d1d69cf38a888d5733ba7a978e33e56f2f1e8))
- relocate source code to agentspec workspace crate ([c118cf1](https://github.com/urmzd/agentspec/commit/c118cf19b361e6c6753af06d35c35ec735e242c8))
- **tui**: update screens for modal-based rendering ([58ab20b](https://github.com/urmzd/agentspec/commit/58ab20be30d9f8b8e952672aeb8b0ad8d11b2769))
- **tui**: migrate from inline modals to modal enum dispatch ([e3e11ec](https://github.com/urmzd/agentspec/commit/e3e11ec76781e6a976f98228e00db6cf5915ab08))
- **tui**: introduce action and modal system ([5dbf117](https://github.com/urmzd/agentspec/commit/5dbf117ccb79572704566f3172bd27d7c259daa6))

### Miscellaneous

- **main,tui**: reformat action handling and ui logic ([dafc830](https://github.com/urmzd/agentspec/commit/dafc83044387188302b1dde8f81aff507e9bef7c))
- **cli,ui,update**: reformat function signatures and expressions ([6bc8419](https://github.com/urmzd/agentspec/commit/6bc8419a66d365a03ac6c2d58d259f30f8dec232))
- **provider**: reformat code for consistency ([c096c6e](https://github.com/urmzd/agentspec/commit/c096c6e202843dbcfe16c989e585676a34249b26))
- **build**: update cargo install path to reflect crate relocation ([bb1350d](https://github.com/urmzd/agentspec/commit/bb1350d3b9379209fb7b59770a163abe16a426e5))
- move resources documentation to docs directory ([3ef0236](https://github.com/urmzd/agentspec/commit/3ef023698c5b50b4e84d7e0e85ae6a3c6c6bb89b))
- **release**: implement ordered crate publishing for workspace ([27bd121](https://github.com/urmzd/agentspec/commit/27bd1213fc675132085cc61d8ed61bdd8d16b166))
- **workspace**: convert single crate to monorepo structure ([2caeb7f](https://github.com/urmzd/agentspec/commit/2caeb7fef2d89cfa05da30070e8395d2d1359535))

[Full Changelog](https://github.com/urmzd/agentspec/compare/v0.5.0...v0.6.0)


## 0.5.0 (2026-04-08)

### Breaking Changes

- **cli,main**: add project subcommand and refactor session handling ([dae4c3e](https://github.com/urmzd/agentspec/commit/dae4c3e5244a28d5eba452bc3932fe4c625da170))
- **session**: add Copilot and Gemini adapters, refactor session module ([da22692](https://github.com/urmzd/agentspec/commit/da22692b24c32ab2d608daab15d8d23a78512841))

### Features

- **tui**: wire up link/unlink and add delete with live refresh ([3d4964a](https://github.com/urmzd/agentspec/commit/3d4964a96bc5f0296215583bff0effa4c9d263da))
- **prune**: add prune command to remove broken resources and stale entries ([52741f4](https://github.com/urmzd/agentspec/commit/52741f47169dde9175e9e7071d25e21dbf59252b))
- **tui**: add configuration list screen ([cc2f6a2](https://github.com/urmzd/agentspec/commit/cc2f6a2065f58d6c498e0c0582d1a6fba4dcf05f))
- **tools**: recognize new session adapters and aliases ([f8a053d](https://github.com/urmzd/agentspec/commit/f8a053d3ef975c2755646374d9724a0d1838e331))
- **ops**: add project config creation and update for new resource kinds ([bd27fa9](https://github.com/urmzd/agentspec/commit/bd27fa9cc8f39ddbde52ecaf5839a4691b8041e5))
- **ops**: add project sync operations ([f55e384](https://github.com/urmzd/agentspec/commit/f55e384272b751415f1b92179ae05b7054093b88))
- **project_files**: static registry of project file specs ([b6cb3f0](https://github.com/urmzd/agentspec/commit/b6cb3f053dcafec297f186fd654afd42e7c139ca))
- **adapters**: add instruction file adapter for editor-specific rules ([48d476d](https://github.com/urmzd/agentspec/commit/48d476d407ad2d4b036c5606c3b0ac50cfac4d1b))
- **config**: add shared project and memory directories ([465dd3a](https://github.com/urmzd/agentspec/commit/465dd3acd628f9e3850892d9cd4eb5c8cc363b7f))
- **ir,inventory**: expand resource model with instruction files and project tracking ([f0bc485](https://github.com/urmzd/agentspec/commit/f0bc4859857e082e8835204ca43951e8cd2628ba))
- **tui**: add info tab showing configuration and tool detection ([5af18b9](https://github.com/urmzd/agentspec/commit/5af18b93500c686c23da5766d15c197008aa0996))
- **ops**: wire up --path parameter through sync pipeline ([6b42e58](https://github.com/urmzd/agentspec/commit/6b42e580ef70d00a477224a487c3619906a47f66))
- **cli**: add --path parameter to status and sync commands ([4f970f2](https://github.com/urmzd/agentspec/commit/4f970f281f3e15fcbc41da4ad25be034c9543e15))

### Bug Fixes

- **tui**: collapse nested if to satisfy clippy ([cca4cc9](https://github.com/urmzd/agentspec/commit/cca4cc996248cbf96a7ca21631f102131a5c4d97))
- **manage**: support @subpath and #branch suffixes for git sources ([ab8108c](https://github.com/urmzd/agentspec/commit/ab8108c8683383de9de3fe38e70af1649070f161))
- **manage**: support @branch suffix for all git sources ([0acf11b](https://github.com/urmzd/agentspec/commit/0acf11b1e6be75b358d217ac5fc0382d63fb58da))
- **discover**: remove unnecessary reference in contains_key calls ([eefa655](https://github.com/urmzd/agentspec/commit/eefa655300c8f85989816bd69fbe21537169907d))

### Documentation

- update showcase assets and demo interactions ([6ba8aed](https://github.com/urmzd/agentspec/commit/6ba8aed9368804ee2423f83628e182f85cd29385))
- add roadmap and update readme ([47363ba](https://github.com/urmzd/agentspec/commit/47363ba11eca0d6927e03ae1be7797cf782c3f02))
- add configuration section to readme ([e34fdc4](https://github.com/urmzd/agentspec/commit/e34fdc4cb8ce14f61234d64b2f095bfad39e7453))
- refactor resource conventions documentation ([c1542c8](https://github.com/urmzd/agentspec/commit/c1542c82028dfdf4abd4cba8ca6fb3fcc4c55ba2))
- refactor README with improved structure and clarity ([a56b065](https://github.com/urmzd/agentspec/commit/a56b0659cdd2b76aedddb20e8e5b192871ebb281))

### Refactoring

- **tui**: improve code formatting in UI modules ([61536d4](https://github.com/urmzd/agentspec/commit/61536d40541fa98adaa37041cf12746ee32b55bd))
- improve code formatting in core and adapter modules ([47bb131](https://github.com/urmzd/agentspec/commit/47bb131533fbe978f92b40c8ff79f49374fb58c7))
- **ops**: improve code formatting and organization ([5e56a00](https://github.com/urmzd/agentspec/commit/5e56a007cd6eabe00bba4edb67a89ca5ed0e6f0b))
- **ops**: use consolidated frontmatter validation ([a14c27a](https://github.com/urmzd/agentspec/commit/a14c27a8abb10ccdb6427d4e50f89f830353612b))
- **discover**: extract walk entry logic and validation helpers ([270d761](https://github.com/urmzd/agentspec/commit/270d761cb4825a021a533900682b2983d05bcbf3))

### Miscellaneous

- bump version to 0.4.1 ([0ccc7cd](https://github.com/urmzd/agentspec/commit/0ccc7cdc6923a2ec8ee6158461c9276ae64fb7c4))
- remove install.sh script ([80421e4](https://github.com/urmzd/agentspec/commit/80421e4a296a959680dc4fc69ba6e8adb451bc02))
- move install script to scripts directory ([447c456](https://github.com/urmzd/agentspec/commit/447c456ae537786736ad352d3ec1b27c5d4117b5))

[Full Changelog](https://github.com/urmzd/agentspec/compare/v0.4.1...v0.5.0)


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
