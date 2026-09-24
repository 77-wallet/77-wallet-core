# Sui JSON-RPC → gRPC 执行计划

状态：已完成，等待真实节点验证。本文保留为迁移执行记录；各批次中的
`migration_XX` 测试名和过滤命令是迁移期间的历史名称，不再是当前测试入口。
当前离线回归统一运行 `cargo test -p wallet-chain-interact --lib sui::provider::tests
--offline`。Sui Foundation 主网 full node 的 JSON-RPC 已关闭。

## dev 黑名单功能 rebase 适配（2026-10-13）

保留 `Provider::coin_deny_status` 和 `black_address`，查询统一走 gRPC：
`LedgerService.GetObject` 读取 `0x403` 的 shared owner 初始版本，构建两个
`0x2::coin::deny_list_v2_contains_*_epoch<T>` 调用，再使用
`TransactionExecutionService.SimulateTransaction`（checks disabled）读取
`command_outputs` 中的 BCS bool。
黑名单模拟请求省略 gas payment，由节点补齐 gas 默认参数，不额外查询
reference gas price，也不选择真实 gas coin。普通估费仍显式传入 gas price。
当前 epoch 表示不能接收，下一 epoch 表示
不能发送，`black_address` 对两者取 OR。对象缺失、非 shared owner、模拟失败
或返回值缺失/格式错误均返回错误。离线验证命令：
`cargo test --offline -p wallet-chain-interact --lib sui::provider::tests`。

## 执行前

1. 阅读根目录及目标目录的 `AGENTS.md`、`docs/codex/testing.md` 和
   `docs/codex/checklists/pr-definition-of-done.md`；检查 `git status --short`。
2. 将本轮批次的范围、验证命令和停止条件同步到根 `PLANS.md`，保留原有内容。
3. 核对当前 Sui 代码。只迁移现存方法，不恢复旧快照中的功能。
4. 迁移执行期间，新增自动测试使用 `migration_01_*` 至 `migration_09_*` 记录所属
   批次。迁移完成后已改为行为导向命名；新增测试继续保持离线，但不再添加批次前缀。
5. 只在用户明确要求时执行公网测试；RPC 地址和认证信息从环境变量读取。

## 固定技术约束

- 使用 crates.io 的 `sui-rpc = { version = "=0.4.0", default-features = false }`。
  直接使用 `sui-sdk-types` 时，使用同版 `=0.4.0`。不启用 `faucet` 或 `unstable`。
- `Provider` 将 gRPC 响应转换为本项目的最小内部类型；protobuf 类型不进入
  `SuiChain`、`builder` 或 `operations` 的公开接口。
- gRPC 请求支持 field mask 时，用它读取必需字段；`GetBalance` 和 `GetCoinInfo`
  请求没有 field mask。缺少业务必需字段时返回错误，不填默认零值。
- `ListOwnedObjects` 的分页 token 是字节串；后续请求保持相同过滤条件和 field mask。
  不能复用 JSON-RPC cursor。
- 所有读取、模拟和提交请求统一走配置的 full-node gRPC endpoint；Provider 不维护
  第二个存档节点连接，也不对 `NotFound` 自动回退。
- 不修改其他链的 `wallet_transport` 用法。原九个 gRPC 迁移批次不替换交易构建/签名使用的
  旧 `sui-types`、`shared-crypto`；这些依赖已在迁移完成后的模块化 SDK 清理批次中移除，
  见文末“后续模块化 SDK 清理”。
- 每个批次先补必要的离线回归测试，再做最小实现。若预计跨两个 crate 或达到 10 个
  文件，按根 `AGENTS.md` 进一步拆批。

## 迁移前实际使用的 JSON-RPC 清单

下表根据迁移前 `wallet-chain-interact/src/sui/provider.rs` 中实际构造的
`JsonRpcParams.method` 整理，只包含本项目真正调用过的 Sui JSON-RPC，不是
Sui JSON-RPC 全量方法列表。共 14 项：5 项 `suix_*`，9 项 `sui_*`。

| 完成 | 原 JSON-RPC 方法 | 原 Provider 入口 | gRPC 替代 | 迁移结果 / 差异 |
| --- | --- | --- | --- | --- |
| [x] | `suix_getBalance` | `balance` | `StateService.GetBalance` | 保留 total balance 业务语义；使用内部 `SuiBalance` |
| [x] | `sui_getLatestCheckpointSequenceNumber` | `latest_block` | `LedgerService.GetServiceInfo` | 读取 `checkpoint_height` 作为最新高度 |
| [x] | `sui_getTransactionBlock` | `query_tx_info` | `LedgerService.GetTransaction` | 用 field mask 读取状态、gas、checkpoint 和时间；历史保留范围由 full node 决定 |
| [x] | `suix_getCoinMetadata` | `get_coin_metadata` | `StateService.GetCoinInfo` | 保留 `decimals` / `symbol` / `name` |
| [x] | `suix_getReferenceGasPrice` | `get_reference_gas_price` | `LedgerService.GetEpoch` | 读取 `reference_gas_price`；零值或缺字段返回错误 |
| [x] | `suix_getOwnedObjects` | `get_owned_objects` | `StateService.ListOwnedObjects` | cursor 从 JSON 字符串改为字节 token；仅保留可明确映射的 `StructType` 过滤 |
| [x] | `sui_getObject` | `get_object_by_id` | `LedgerService.GetObject` | 只映射 object reference 所需字段 |
| [x] | `suix_getCoins` | `get_all_coins_by_owner` | `StateService.ListOwnedObjects` | 改用精确 `0x2::coin::Coin<T>` 过滤并遍历字节分页 token |
| [x] | `sui_devInspectTransactionBlock` | `dev_inspect_transaction` | `TransactionExecutionService.SimulateTransaction` | 传 transaction kind，`checks = Disabled`，不自动选 gas |
| [x] | `sui_dryRunTransactionBlock` | `dry_run_transaction` | `TransactionExecutionService.SimulateTransaction` | 传完整 `TransactionData` BCS，`checks = Enabled` |
| [x] | `sui_getCheckpoint` | `get_check_point` | `LedgerService.GetCheckpoint` | 支持 sequence number 或 digest |
| [x] | `sui_multiGetTransactionBlocks` | `get_multi_trans` | `LedgerService.BatchGetTransactions` | 保持输入顺序并逐项检查；任一项失败则整体返回错误 |
| [x] | `sui_executeTransactionBlock` | `send_transaction` | `TransactionExecutionService.ExecuteTransaction` | 直接传 BCS/签名字节，不再 base64 往返；确认执行成功后才返回 digest |
| [x] | `sui_getNormalizedMoveModulesByPackage` | `get_normalized_move_modules_by_package_id` | `MovePackageService.GetPackage` | **不完全等价**；gRPC 返回原始模块字节码，入口已改为 `get_package_modules` 并返回 `SuiPackage` |

核对结论：14 个旧 JSON-RPC 调用均已移出 Sui 生产代码。其中
`suix_getOwnedObjects`、`suix_getCoins` 合并到同一 gRPC 列表能力；
dev-inspect 和 dry-run 合并到同一模拟方法；normalized Move module 查询不存在
完全等价的 gRPC 返回，已明确改为原始字节码接口。

## 与现有 JSON-RPC 实现的兼容性基线

| 现有入口或行为 | 迁移目标 | 兼容性验收 |
| --- | --- | --- |
| `SuiChain::balance`、`block_num`、`decimals`、`token_symbol`、`token_name` | 保留签名和业务值 | 对同一输入，余额总额、checkpoint 高度和币种元数据一致 |
| `SuiChain::query_tx_res` | 保留签名及 `QueryTransactionResult` 字段含义 | 同一交易的状态、时间、checkpoint 和手续费口径一致；历史可查询范围取决于 full node |
| `SuiChain::estimate_fee` | 保留签名、`EstimateFeeResp` 和现有估费公式 | 相同模拟结果得到相同 gas fee 与 gas price；模拟失败继续返回错误 |
| `SuiChain::exec` | 保留签名，成功时仍返回 digest | 链上执行失败改为返回错误，不能继续把失败交易的 digest 当作成功结果 |
| `Provider::new(RpcClient)` 及返回的 `Balance`、`Coin`、`SuiTransactionBlockResponse` 等旧 DTO | 最终改用 gRPC 构造入口和内部类型 | **Rust 源码不兼容**；批次 09 更新所有仓内调用点和使用说明 |
| `get_owned_objects(filter, cursor, limit)` | 仅支持可明确映射的过滤器；gRPC 使用字节分页 token | 不支持的过滤器明确报错；旧 cursor 不可复用，公开接口变化须记录 |
| `get_multi_trans` | 保持输入顺序；逐项检查 batch 结果 | 任一项失败则返回错误，不返回遗漏项目的部分 `Vec` |
| `get_normalized_move_modules_by_package_id` | 尽量保持完整模块信息 | 如果 `GetPackage` 不能提供原字段，明确修改返回类型并记录调用方影响 |

旧 JSON-RPC 的 `TransRespOpt`、对象过滤器和响应 DTO 不等同于 gRPC 的 field mask、
类型过滤器和 protobuf 消息。只保证上表明确列出的业务语义；公开 Rust 类型变化在
批次 09 验收时逐项检查。

## 批次 01：测试隔离与客户端入口

范围：根 `Cargo.toml`、`wallet-chain-interact/Cargo.toml`、
`wallet-chain-interact/src/sui/provider.rs`。

1. 给 `provider.rs` 中所有访问 testnet 的现有测试加 `#[ignore]`，注明手动网络测试。
2. 引入 `sui-rpc`。最终 `Provider` 只持有一个 full-node gRPC client。
3. 暂留 `Provider::new(RpcClient)`，新增明确接收 gRPC 配置的构造入口。旧入口只供尚未
   迁移的方法使用；到批次 09 删除。为客户端配置和构造错误补离线测试。

验证：

```bash
cargo test -p wallet-chain-interact --lib migration_01
cargo check -p wallet-chain-interact
```

停止条件：客户端可编译；公网测试默认不运行；新增离线测试通过。本批不改业务调用。

## 批次 02：余额与币种元数据

范围：`wallet-chain-interact/src/sui/{provider,protocol,chain}.rs`。

1. `balance` → `StateService.GetBalance`，读取包含 coin object 和 address balance 的
   total balance。
2. `get_coin_metadata` → `StateService.GetCoinInfo`，保持 `decimals`、`symbol`、
   `name` 的上层语义。
3. 定义这两个方法所需的最小内部返回类型；测试正常和缺字段响应。

验证：

```bash
cargo test -p wallet-chain-interact --lib migration_02
cargo check -p wallet-chain-interact
```

停止条件：上述两个方法和上层调用均不再使用 JSON-RPC 返回类型。

## 批次 03：coin 读取与选币

范围：`wallet-chain-interact/src/sui/{provider,protocol,builder}.rs`；类型适配需要时，
可修改 `operations/transfer.rs`。

1. `get_all_coins_by_owner` → `StateService.ListOwnedObjects`，使用
   `0x2::coin::Coin<T>` 过滤，逐页读取 object ID、version、digest 和 balance。
2. 将 coin 转成选币和 `ObjectRef` 构造需要的内部类型。测试多页、缺字段和重复 token。
3. 核对 `get_owned_objects` 的实际调用。将可支持的过滤器映射到 gRPC；无法等价表达
   的过滤器返回明确错误，并记录公开接口变化。

验证：

```bash
cargo test -p wallet-chain-interact --lib migration_03
cargo check -p wallet-chain-interact
```

停止条件：coin 读取与选币不再调用 JSON-RPC；转账构建通过编译。

## 批次 04：checkpoint 与 object 点查

范围：`wallet-chain-interact/src/sui/{provider,protocol,chain}.rs`。

1. `latest_block` → `LedgerService.GetServiceInfo.checkpoint_height` 或获取最新
   checkpoint 的 `GetCheckpoint`；保持 `block_num()` 返回值语义。
2. `get_check_point` → `LedgerService.GetCheckpoint`；`get_object_by_id` →
   `LedgerService.GetObject`。只映射当前调用方需要的字段。
3. `NotFound` 直接作为 full node 查询结果返回，不配置第二个查询端点。

验证：

```bash
cargo test -p wallet-chain-interact --lib migration_04
cargo check -p wallet-chain-interact
```

停止条件：上述方法不再调用 JSON-RPC，所有查询统一使用 full node。

## 批次 05：交易查询

范围：`wallet-chain-interact/src/sui/{provider,protocol,chain}.rs`。

1. `query_tx_info` → `LedgerService.GetTransaction`；`get_multi_trans` →
   `LedgerService.BatchGetTransactions`。`sui-rpc 0.4.0` 已提供 batch 客户端方法。
2. 对 batch 的每个 `GetTransactionResult` 分别处理成功或错误，保持与输入 digest 的
   对应关系；任一项失败时返回错误，不静默返回部分 `Vec`。
3. 将交易状态、gas、checkpoint 和时间映射为内部类型。测试成功、执行失败、缺字段和
   batch 部分失败。

验证：

```bash
cargo test -p wallet-chain-interact --lib migration_05
cargo check -p wallet-chain-interact
```

停止条件：交易查询不再调用 JSON-RPC，并统一使用 full node。

## 批次 06：参考 gas 价格与模拟

范围：`wallet-chain-interact/src/sui/{provider,protocol,chain}.rs`。

1. `get_reference_gas_price` → `LedgerService.GetEpoch.reference_gas_price`。
2. `dry_run_transaction` 和 `dev_inspect_transaction` →
   `TransactionExecutionService.SimulateTransaction`。分别核对完整交易与仅包含交易
   kind 的输入、`checks`、gas selection 和 field mask。
3. 保持当前估费计算口径。测试模拟成功、Move abort、必要字段缺失和 BCS 输入编码。
   当前代码没有 deny-list 检查，无需增加。

验证：

```bash
cargo test -p wallet-chain-interact --lib migration_06
cargo check -p wallet-chain-interact
```

停止条件：gas price、dry-run、dev-inspect 和估费不再调用 JSON-RPC。

## 批次 07：Move package 查询

范围：`wallet-chain-interact/src/sui/{provider,protocol}.rs`，以及使用该返回值的
Sui 调用方。

1. `get_normalized_move_modules_by_package_id` → `MovePackageService.GetPackage`。
2. 核对新响应能否完整提供旧的 normalized module map。能提供则转换并测试；不能
   提供则定义准确的新返回类型，更新调用方并在模块文档记录兼容性变化。

验证：

```bash
cargo test -p wallet-chain-interact --lib migration_07
cargo check -p wallet-chain-interact
```

停止条件：package 查询不再调用 JSON-RPC；返回类型不存在未声明的字段缺失。

## 批次 08：交易提交

范围：`wallet-chain-interact/src/sui/{provider,chain}.rs`。

1. `send_transaction` → `TransactionExecutionService.ExecuteTransaction`。核对现有交易
   BCS 和签名字节的编码，不进行二次 base64 或重复 intent 编码。
2. 返回 digest 前检查执行结果。离线测试覆盖成功、链上执行失败和传输失败。
3. 不对提交配置第二节点回退；重试前必须确认该错误对重复提交安全。

验证：

```bash
cargo test -p wallet-chain-interact --lib migration_08
cargo check -p wallet-chain-interact
```

停止条件：交易提交不再调用 JSON-RPC；失败交易不会被报告为成功。

## 批次 09：删除 Sui JSON-RPC 兼容路径

范围：`wallet-chain-interact` 的 Sui 模块、该 crate 的依赖声明和 Sui 模块文档。

1. 删除 `Provider` 的 `RpcClient`、`JsonRpcParams`、旧构造入口及剩余 JSON-RPC
   请求；移除 Sui 代码对 `sui-json-rpc-types` 的引用与直接依赖。
2. 更新 Sui Provider 构造示例；检查已忽略的手动网络测试是否使用 gRPC。
3. 用 `cargo tree -d` 记录仍由 `sui-types`、`shared-crypto` 和 `wallet-utils` 带入的旧
   Sui SDK / Git 依赖。其全面替换另立计划，不作为本批完成条件。

验证：

```bash
rg -n 'sui_json_rpc_types|JsonRpcParams|sui_(get|multi|dry|dev|execute)|suix_' wallet-chain-interact/src/sui
cargo test -p wallet-chain-interact --lib sui::provider::tests
cargo check -p wallet-chain-interact
cargo tree -p wallet-chain-interact -d
```

停止条件：`rg` 在 Sui 生产代码中无命中；离线测试和 crate check 通过；旧依赖来源
已记录。

构造示例（需在 Tokio runtime 内调用）：

```rust
let provider = wallet_chain_interact::sui::Provider::new(
    "https://your-sui-grpc-endpoint",
)?;
let chain = wallet_chain_interact::sui::SuiChain::new(provider)?;
```

手动联调测试独立为 `cargo test -p wallet-chain-interact --test sui --offline`；14 个
Provider 请求用例默认全部忽略，不会访问节点。要运行指定的只读用例，先在运行环境
设置 `SUI_TESTNET_GRPC_URL`，再使用 `--test sui <测试名> -- --ignored`。地址、交易、
object、coin type、checkpoint 和 package 均可用对应的 `SUI_TEST_*` 环境变量覆盖。
模拟和 dry-run 还需 `SUI_TEST_FUNDED_ADDRESS`；真实提交额外需要与该地址匹配的
`SUI_TESTNET_PRIVATE_KEY`，会产生链上交易。仅可使用专门的测试网密钥，不应在命令行
或仓库文件中写入私钥。

本次删除 `Provider::new(RpcClient)`、过渡用 `Provider::with_grpc`、`TransRespOpt`
及三个 `SuiTransactionBlock*` JSON-RPC DTO 导出；`query_tx_info` 和
`get_multi_trans` 不再接收无效的 `TransRespOpt` 参数。下游 Rust 调用方须按上述入口
改造；构造现会校验端点并返回 `Result`。这属于公开 Rust 源码不兼容变更。

## 完成与记录

全部批次完成后，按 PR DoD 检查业务行为、文档、测试和公开 API 变化。每批在下面记录
修改文件、实际执行的命令与测试数、结果和剩余风险。不得将通过 `cargo check` 等同于
真实 gRPC endpoint 联通；上线前还需用专用 endpoint 手动验证读取、模拟和提交。

| 批次 | 状态 | 修改文件 / 验证结果 / 风险 |
| --- | --- | --- |
| 01 | 已完成 | `Cargo.toml`、`wallet-chain-interact/Cargo.toml`、`wallet-chain-interact/src/sui/provider.rs`；`cargo test -p wallet-chain-interact --lib migration_01 --offline`：4 通过；`cargo test -p wallet-chain-interact --lib sui::provider::tests --offline`：4 通过、6 个手动公网测试忽略；`cargo check -p wallet-chain-interact --offline` 通过。gRPC 构造要求 Tokio runtime；业务 RPC 尚未迁移。 |
| 02 | 已完成 | `wallet-chain-interact/src/sui/{provider,protocol}.rs`：余额与币种元数据改走 `StateService`，返回内部类型；`SuiChain` 原字段访问无需改动。`cargo test -p wallet-chain-interact --lib migration_02 --offline`：5 通过；`cargo check -p wallet-chain-interact --offline` 通过。未做真实 endpoint 验证；旧 `Provider::new` 无 gRPC 配置，调用这两个方法会返回配置错误；两个 Provider 返回类型发生 Rust 源码变化。 |
| 03 | 已完成 | `wallet-chain-interact/src/sui/{provider,protocol,builder}.rs`：coin 读取改用 `ListOwnedObjects` 的精确 `Coin<T>` 过滤、固定 read mask、字节分页 token，选币改用内部 `SuiCoin`；`Object.balance` 已是可选 coin 余额字段，无需读取 BCS。无仓内调用的 `get_owned_objects` 支持无过滤或 `StructType`，其他过滤器明确报错；其 cursor、limit 和返回类型发生 Rust 源码变化，且返回最小对象字段。`cargo test -p wallet-chain-interact --lib migration_03 --offline`：5 通过；`cargo check -p wallet-chain-interact --offline`、格式与 diff 检查通过。未做真实 endpoint 验证。 |
| 04 | 已完成 | checkpoint/object 点查迁至 `LedgerService`，使用最小内部 DTO。迁移期间曾实现第二节点回退，后续按当前 Provider 设计移除；查询统一使用 full node。 |
| 05 | 已完成 | 单笔/批量交易查询改用 `LedgerService.GetTransaction` / `BatchGetTransactions`，读取 digest、执行状态、gas、checkpoint 和时间；批量逐项校验且不返回部分结果。查询统一使用 full node。 |
| 06 | 已完成 | `wallet-chain-interact/Cargo.toml`、`wallet-chain-interact/src/sui/{provider,protocol,chain}.rs`：参考 gas 价格迁至 `LedgerService.GetEpoch`；完整交易 dry-run 以 `TransactionData` BCS 及 checks enabled 模拟，dev-inspect 将旧 transaction kind BCS 转为同版 `sui-sdk-types =0.4.0` 和结构化 gRPC transaction，以 checks disabled 模拟；两者使用固定效果字段 mask，不做 gas selection。Provider 转为最小 `SuiSimulation`；估费维持计算成本加存储成本、零值回退默认预算，Move abort 返回错误。`dry_run_transaction` / `dev_inspect_transaction` 返回类型变更；旧 kind BCS 的跨版兼容只经离线样例验证，真实节点行为待验证。`cargo test -p wallet-chain-interact --lib migration_06 --offline`：6 通过；`cargo test -p wallet-chain-interact --lib migration_0 --offline`：32 通过；`cargo check -p wallet-chain-interact --offline` 通过。 |
| 07 | 已完成 | `wallet-chain-interact/src/sui/{provider,protocol}.rs`：Move package 查询改用 `MovePackageService.GetPackage`。SDK 0.4.0 的 module 响应包含原始字节码及部分描述符，不能直接无损替代旧 `SuiMoveNormalizedModule` 的完整 normalized map；原入口改名为 `get_package_modules`，返回含 storage/original ID、版本和按名称索引的原始模块字节码的 `SuiPackage`，不伪装为 normalized 数据。仓内无业务调用方，手动测试已适配；下游调用方需自行适配，这是 Rust 源码兼容性变化。缺失 ID/版本/模块名/字节码、重复模块名及 ID 不匹配均报错。`cargo test -p wallet-chain-interact --lib migration_07 --offline`：3 通过；累计 `migration_0`：35 通过；`cargo check -p wallet-chain-interact --offline`、格式与 diff 检查通过。未做真实 endpoint 验证。 |
| 08 | 已完成 | `wallet-chain-interact/src/sui/{provider,chain}.rs`：提交改用 `TransactionExecutionService.ExecuteTransaction`，直接传原始 `TransactionData` BCS 和 `UserSignatureBytes`，无 base64 往返或额外 intent 编码；显式读取 digest 与 `effects.status`，只在执行成功且 digest 有效时返回。链上执行失败、字段缺失及传输错误均返回错误；不使用第二节点回退或自动重试，传输中断时结果可能未知，须先按 digest 查询再决定是否重试。`SuiChain::exec` 签名和成功值保持，`Provider::send_transaction` 的入参/返回类型发生 Rust 源码变化；旧 JSON-RPC client 字段目前未使用，留给批次 09 删除。`cargo test -p wallet-chain-interact --lib migration_08 --offline`：5 通过；累计 `migration_0`：40 通过；`cargo check -p wallet-chain-interact --offline`、格式与 diff 检查通过。未做真实 endpoint 验证。 |
| 09 | 已完成（待节点验证） | 根与 `wallet-chain-interact` 的 Cargo manifest、`wallet-chain-interact/src/sui/{provider,chain,mod}.rs`、本迁移文档与索引：移除 Sui JSON-RPC 直接依赖、旧 Provider 构造与无效查询选项，手动测试改为从 `SUI_TESTNET_GRPC_URL` 构造 gRPC Provider。`cargo test -p wallet-chain-interact --lib migration_ --offline`：39 通过；`cargo check -p wallet-chain-interact --offline`、残留符号扫描、格式与 diff 检查通过。`cargo tree -p wallet-chain-interact -d --offline` 仍显示旧 Git 版 `sui-types`、`shared-crypto` 和其传递的 `sui-sdk-types 0.0.4`，同时使用 crates.io 的 `sui-sdk-types 0.4.0`；旧版 `sui-sdk-types` 由 `sui-types` 直接带入，`wallet-utils` 也依赖该 `sui-types`。`sui-json-rpc-types` 已不在该 crate 依赖图中。未做真实 gRPC endpoint 的读取、模拟、提交验证，不可据此认定上线就绪。 |

### 后续模块化 SDK 清理

JSON-RPC → gRPC 九个批次完成后，又分三批将应用层从 Sui 节点仓库的
`sui-types` / `shared-crypto` 迁到模块化 SDK 0.4：

1. 签名改用 `sui-crypto 0.4.0`，只开启 `ed25519`；无效私钥长度返回错误而不再 panic。
2. `wallet-chain-interact` 的 object reference、PTB、transaction、模拟与提交类型改用
   `sui-sdk-types 0.4.0`；SUI 和 `Coin<T>` 转账的迁移前后 BCS 字节一致。
3. `wallet-utils` 的 Sui `Address` / `TypeTag` 解析改用 `sui-sdk-types 0.4.0`，
   删除根 workspace 和各 crate 对旧 Git `sui-types` / `shared-crypto` 的声明。

最终 `wallet-chain-interact` 的 Sui 直接依赖只保留同版的 `sui-rpc 0.4.0`、
`sui-sdk-types 0.4.0` 和 `sui-crypto 0.4.0`。依赖树及 `Cargo.lock` 均无旧
`sui-types`、`shared-crypto` 或 `sui-sdk-types 0.0.4`。迁移收尾时的离线验证结果为
43 个迁移测试通过；这些测试随后改为行为导向命名。`wallet-utils` 和
`wallet-chain-interact` crate check、格式与 diff 检查通过。真实 gRPC endpoint 验证
仍是上线前必做项。


## Recharge 插件扫块字段补齐（2026-10-13）

`get_check_point` 的 `CheckpointResult` 新增有序 `transactions: Vec<String>`，
读取 `contents.transactions.transaction`。`query_tx_info` 和 `get_multi_trans`
共用的 `SuiQueriedTransaction` 新增 `gas_owner: String` 与
`balance_changes: Vec<SuiBalanceChange>`；后者包含规范化的 `address`、`coin_type`
及最小单位有符号 `amount: i128`。读取 mask 同时包含 gas owner 和余额变更。
余额变更包含 SUI gas 扣减，扫描方应只对 gas owner 的 SUI delta 加回净手续费，
手续费可能因 storage rebate 为负。缺失或非法的必需字段返回错误；批量任一项
失败仍整体返回错误。空余额变更和空 checkpoint contents 交易列表是合法结果。
新增字段使手动构造这些内部 DTO 的 Rust 调用方需要同步更新 struct literal。
插件统一复用 Provider 的 full-node 连接，无额外 protobuf/client 依赖。
