portfolio-cli
==============

Track balance of ETH and ERC20 tokens easily from cli


### Usage

- Clone the repository
- You need a Settings.toml file in the root directory with an [Infura API key](https://infura.io/docs/gettingStarted/authentication) with this format:

```
key = <infura-api-key>
```

- Then run in the command line:

```
$ cargo build
$ cargo run <wallet-address>
```
