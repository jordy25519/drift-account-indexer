# drift-indexer

Monitor solana for Drift account events and persist them into storage.   

```console
$> ./indexer --help

Drift account indexing service 🏎️

Usage: indexer [OPTIONS]

Options:
      --accounts <ACCOUNTS>  List of accounts to monitor
      --db <DB>              Db connection string
      --rpc <RPC>            Solana RPC endpoint
      --poll <POLL>          Polling interval (seconds) [default: 3]
  -h, --help                 Print help
```

## Usage
```console
$> indexer \
    --accounts BTDXiRzG1QBP7bfK4A33RcSP5mmZx8mGJ9YC5maetoD6,GontTwDeBduvbW85oHyC8A7GekuT8X1NkZHDDdUWWvsV 
    --poll 10
    --rpc <RPC_URL>
    --db mongodb://localhost:27017
```

## Build & Run
```console
docker-compose up --build
```
NB: if quickly hits rate limits on free RPC, try increasing `--poll` seconds, or use a 3rd party provider

## Future work
- Add client side rate-limiting
- db tuning needs some work (indexes, data model), test under more load
- at some point subscribing to _N_ accounts is going to be less efficient than simply subscribing to all drift trades
