# drift-indexer

Watches solana for drift account events and stores them.   

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

## TODO:
- [x] figure out DB schema and writing
- [x] add cli flags/args
    - [x] --accounts=<address1>,<address2>
- [x] create `Dockerfile` + `docker-compose` file for testing
- [x] store last processed signature
- [] write a note about the DB code, not quite right

out of scope
- [] handle all event drift event types e.g. with strategy/visitor pattern
- [ ] check drift program version at start up