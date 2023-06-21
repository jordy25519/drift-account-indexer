# idl-gen

Generate rust types from anchor IDL, intended for offchain services.
For cpi see [anchor-gen](https://github.com/saber-hq/anchor-gen/)

```rust
// import deps to enable ser/de
use anchor_attribute_event::event;
use anchor_lang::prelude::*;
use serde::{Serialize, Deserialize};
// + any other external type defs (macro will not resolve)
use solana_sdk::pubkey::Pubkey;

gen_idl_types!("../rel/path/to/idl.json");
```

## Why not use the original anchor code?
- Using source would cause unnecessary build complexity/coupling  
- Can't (or don't want to) build the source  
- working with multiple program IDLs

## TODO:
- [] field names are camelCase, should become snake_case
