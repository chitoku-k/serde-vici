Serde VICI
==========

[![][workflow-badge]][workflow-link]
[![][docsrs-badge]][docsrs-link]
[![][cratesio-badge]][cratesio-link]

This crate is a Rust library for using the [Serde][] serialization framework
with data in the [VICI][] protocol format.

## Dependency

To make best use of this crate, let Serde's derive macros handle structs in your
application.

```toml
[dependencies]
serde = { version = "1.0", features = ["derive"] }
serde_vici = "0.1"
```

## Using Serde VICI

For example, serializing/deserializing the [Encoding Example][] looks like the
following:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, PartialEq, Serialize)]
struct RootSection {
    key1: String,
    section1: MainSection,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
struct MainSection {
    sub_section: SubSection,
    list1: Vec<String>,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
struct SubSection {
    key2: String,
}

fn main() -> Result<(), serde_vici::Error> {
    // Define a struct as in the documentation for the VICI protocol.
    let data = RootSection {
        key1: "value1".to_string(),
        section1: MainSection {
            sub_section: SubSection {
                key2: "value2".to_string(),
            },
            list1: vec!["item1".to_string(), "item2".to_string()],
        },
    };

    // Serialize to a vector.
    let msg = serde_vici::to_vec(&data)?;
    assert_eq!(
        msg,
        vec![
            // key1 = value1
            3, 4, b'k', b'e', b'y', b'1', 0, 6, b'v', b'a', b'l', b'u', b'e', b'1',
            // section1
            1, 8, b's', b'e', b'c', b't', b'i', b'o', b'n', b'1',
            // sub-section
            1, 11, b's', b'u', b'b', b'-', b's', b'e', b'c', b't', b'i', b'o', b'n',
            // key2 = value2
            3, 4, b'k', b'e', b'y', b'2', 0, 6, b'v', b'a', b'l', b'u', b'e', b'2',
            // sub-section end
            2,
            // list1
            4, 5, b'l', b'i', b's', b't', b'1',
            // item1
            5, 0, 5, b'i', b't', b'e', b'm', b'1',
            // item2
            5, 0, 5, b'i', b't', b'e', b'm', b'2',
            // list1 end
            6,
            // section1 end
            2,
        ]
    );

    // Deserialize back to a Rust type.
    let deserialized_data: RootSection = serde_vici::from_slice(&msg)?;
    assert_eq!(data, deserialized_data);
    Ok(())
}
```

[workflow-link]:    https://github.com/chitoku-k/serde-vici/actions?query=branch:master
[workflow-badge]:   https://img.shields.io/github/actions/workflow/status/chitoku-k/serde-vici/ci.yml?branch=master&style=flat-square&logo=github
[docsrs-link]:      https://docs.rs/serde_vici/
[docsrs-badge]:     https://img.shields.io/docsrs/serde_vici?style=flat-square
[cratesio-link]:    https://crates.io/crates/serde_vici
[cratesio-badge]:   https://img.shields.io/crates/v/serde_vici?style=flat-square
[Serde]:            https://github.com/serde-rs/serde
[VICI]:             https://github.com/strongswan/strongswan/blob/5.9.5/src/libcharon/plugins/vici/README.md
[Encoding Example]: https://github.com/strongswan/strongswan/blob/5.9.5/src/libcharon/plugins/vici/README.md#encoding-example
