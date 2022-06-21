use figment::{Provider, Metadata, Profile, Error};
use figment::value::{Map, Dict, Value, Tag};

use crate::config::ArgumentTree;

/// A provider that fetches its data from a given URL.
pub struct CliProvider {
    /// The profile to emit data to if nesting is disabled.
    profile: Option<Profile>,
    args: Vec<std::string::String>,
}

impl CliProvider {
    pub fn new() -> CliProvider {
        CliProvider {
            profile: None,
            args: wild::args().collect(),
        }
    }
}

impl Provider for CliProvider {
    /// Returns metadata with kind `Cli Flags`, custom source is the 
    /// command line arguments separated by spaces.
    fn metadata(&self) -> Metadata {
        let args = &self.args;
        Metadata::named("Cli Flags")
            .source(args.join(" "))
    }

    /// Parses the command line arguments into a `Map` and `Value`s.
    fn data(&self) -> Result<Map<Profile, Dict>, Error> {
        // Parse a `Value` from a `String` 
        fn parse_from_string(string: &String) -> Value {
            // TODO: Other integer types
            match string.parse::<i32>() {
                Ok(i) => Value::Num(Tag::Default, figment::value::Num::I32(i)),
                Err(_) => match string.parse::<bool>() {
                    Ok(b) => Value::Bool(Tag::Default, b),
                    Err(_) => Value::from(string.to_owned()),
                },
            }
        }

        fn parse_cli(args: &Vec<std::string::String>)-> Result<Dict, Error> {
            // TODO: Parse _args as booleans
            let (_args, argv) = argmap::parse(args.iter());

            let mut dict = Dict::new();
            let mut tree = ArgumentTree::new();
            for (key, vals) in argv {
                let len = vals.len();
                if len == 0 {
                    // This is usually where booleans are given (use_cache, replace_torrents, etc.)
                    dict.insert(key, Value::Bool(Tag::Default, true));

                    continue;
                }

                // Parse the string argument values into a `Value`
                let val = match len {
                    1 => parse_from_string(&vals[0]),
                    _ => {
                        let mut vec = Vec::new();
                        for val in vals {
                            vec.push(parse_from_string(&val));
                        }
                        Value::from(vec)
                    },
                };

                // Separate the key into its parts and then insert it into the tree
                let key_vec = key.split(".").map(|s| s.to_string()).collect::<Vec<_>>();
                let mut key_iter = key_vec.iter();
                tree.insert(&mut key_iter, val);
            }

            dict.append(&mut tree.to_dict());

            Ok(dict)
        }

        match &self.profile {
            // Don't nest: `fetch` into a `Dict`.
            Some(profile) => Ok(profile.collect(parse_cli(&self.args)?)),
            None => {
                let mut map = Map::new();
                map.insert(Profile::default(), parse_cli(&self.args)?);
                Ok(map)   
            }
        }
    }
}