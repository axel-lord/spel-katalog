//! Usage example.

use ::core::panic;
use ::std::collections::HashMap;

fn main() {
    let mut args = ::std::env::args();
    _ = args.next();

    let format = args
        .next()
        .unwrap_or_else(|| panic!("missing format argument"));

    let mut map = HashMap::new();
    let mut arr = Vec::new();
    let mut counter = 0usize;

    let args = args.collect::<Vec<_>>();

    for arg in &args {
        if let Some((key, value)) = arg.split_once('=') {
            map.insert(key, value);
        } else {
            arr.push(arg.as_str());
        }
    }

    let formatted = ::spel_katalog_parse::interpolate_str(&format, |key| {
        if key.is_empty() {
            let value = arr.get(counter);
            counter += 1;
            value
        } else if let Ok(idx) = key.parse::<usize>() {
            arr.get(idx)
        } else {
            map.get(key)
        }
        .map(|v| *v)
    })
    .unwrap_or_else(|err| panic!("{err}"));

    println!("{formatted}")
}
