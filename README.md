portfolio-cli
==============

Track balance of ETH and ERC20 tokens easily from cli

[![codecov](https://codecov.io/gh/JonaC22/portfolio-cli/branch/master/graph/badge.svg?token=LIJC61SRHC)](https://codecov.io/gh/JonaC22/portfolio-cli)

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
- You need a `Settings.toml` file in the root directory with an [Infura API key](https://infura.io/docs/gettingStarted/authentication), [Etherscan API key](https://info.etherscan.com/etherscan-developer-api-key/), and [Ethplorer API key](https://github.com/EverexIO/Ethplorer/wiki/ethplorer-api) with this format:

```
infura = <infura-api-key>
etherscan = <etherscan-api-key>
```

- Then run in the command line:

```
$ cargo build
$ cargo run -- -a <wallet-address>
```

- Can also run verbose mode with:

```
$ cargo run -- -a <wallet-address> -v
```

- For more options run:

```
$ cargo run -h
```

### Testing

- For testing, you will need the api keys suitable for that, add it to `Settings.toml`:

```
...
test_etherscan = <etherscan-api-key>
test_ethplorer = <ethplorer-api-key>
```

- Then run:

```
$ cargo test
```

### Coverage

For coverage tests, install and run [tarpaulin](https://github.com/xd009642/tarpaulin)
