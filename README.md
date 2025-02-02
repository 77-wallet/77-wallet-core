# 77 Wallet Core Library

This is the core library of the 77 Wallet SDK, implemented in Rust. It provides core chain abstractions and wallet keystore management functionalities. Rust ensures that the library is both safe and high-performance.

## Installation and Usage

This library is a low-level foundational library. If you want to simply create a wallet, you can directly use the [77-wallet-sdk](https://github.com/77-wallet/77-wallet-sdk). If you wish to create a new wallet application based on this library, you can refer to the [77-wallet-sdk](https://github.com/77-wallet/77-wallet-sdk).


## Project Structure

### [`wallet-chain-instance`](wallet-chain-instance)

This subproject contains implementations of chain instances, providing concrete implementations for different blockchains.

### [`wallet-chain-interact`](wallet-chain-interact)

This subproject provides functionalities to interact with blockchains, including creating and sending transactions.

### [`wallet-core`](wallet-core)

This subproject is the core of the entire SDK, containing the main logic and functionalities related to key derivation.

### [`wallet-keystore`](wallet-keystore)

This subproject is responsible for managing the wallet's keystore, providing functionalities for key generation, storage, and management.

### [`wallet-transport`](wallet-transport)

This subproject provides functionalities to communicate with blockchain nodes, handling the sending and receiving of blockchain data.

### [`wallet-tree`](wallet-tree)

This subproject manages the hierarchical structure of the wallet, including the management of root and child keys.

### [`wallet-types`](wallet-types)

This subproject defines various types and data structures used in the SDK.

### [`wallet-utils`](wallet-utils)

This subproject provides utility functions used by other subprojects.

## Supported Versions
77-wallet-core aims to support the latest stable versions of the languages and tools it integrates with. Make sure to check the compatibility requirements for each module in their respective documentation.

## Contributing
Thank you for your interest in contributing to 77-wallet-core! We welcome contributions from the community. Please refer to our contributing guide for guidelines on how to get involved.

Pull requests will not be merged unless they pass all CI checks. Please ensure that your code follows the project's style guidelines and passes all tests.

## Note on Platform Compatibility
While 77-wallet-core is designed to be cross-platform, certain modules may have platform-specific requirements or limitations. Refer to the documentation of each module for detailed compatibility information.

## Credits
77-wallet-core builds upon the work of numerous open-source projects and libraries. We acknowledge and thank the developers of these projects for their contributions to the open-source community.

## License
Licensed under either of Apache License, Version 2.0 or MIT license at your option. Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in 77-wallet-core by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
