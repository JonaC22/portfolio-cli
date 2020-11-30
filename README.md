portfolio-cli
==============

Track balance of ETH and ERC20 tokens easily from cli

### Features

- List ETH and ERC20 total balance in ETH / USD
- Show portfolio total balance in pie chart

### Requirements

You will need some libs installed in your OS (Tested on Ubuntu 20.04):

- gcc
- libc-dev
- libssl-dev

### Usage

- Clone the repository
- You need a `Settings.toml` file in the root directory with an [Infura API key](https://infura.io/docs/gettingStarted/authentication) and [Etherscan API key](https://info.etherscan.com/etherscan-developer-api-key/) with this format:

```
infura = <infura-api-key>
etherscan = <etherscan-api-key>
```

- Then run in the command line:

```
$ cargo build
$ cargo run <wallet-address>
```

### Testing

- For testing, you will need an etherscan api key suitable for that, add it to `Settings.toml`:

```
...
test_etherscan = <etherscan-api-key>
```

- Then run:

```
$ cargo test
```
