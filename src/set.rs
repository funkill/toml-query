/// The Toml Set extensions

#[cfg(feature = "typed")]
use serde::Serialize;
use toml::Value;

use crate::error::{Error, Result};
use crate::tokenizer::tokenize_with_seperator;
use crate::tokenizer::Token;

pub trait TomlValueSetExt {
    /// Extension function for setting a value in the current toml::Value document
    /// using a custom seperator
    ///
    /// # Semantics
    ///
    /// The function _never_ creates intermediate data structures (Tables or Arrays) in the
    /// document.
    ///
    /// # Return value
    ///
    /// * If the set operation worked correctly, `Ok(None)` is returned.
    /// * If the set operation replaced an existing value `Ok(Some(old_value))` is returned
    /// * On failure, `Err(e)` is returned:
    ///     * If the query is `"a.b.c"` but there is no table `"b"`: error
    ///     * If the query is `"a.b.[0]"` but "`b"` is not an array: error
    ///     * If the query is `"a.b.[3]"` but the array at "`b"` has no index `3`: error
    ///     * etc.
    ///
    fn set_with_seperator(&mut self, query: &str, sep: char, value: Value)
        -> Result<Option<Value>>;

    /// Extension function for setting a value from the current toml::Value document
    ///
    /// See documentation of `TomlValueSetExt::set_with_seperator`
    fn set(&mut self, query: &str, value: Value) -> Result<Option<Value>> {
        self.set_with_seperator(query, '.', value)
    }

    /// A convenience method for setting any arbitrary serializable value.
    #[cfg(feature = "typed")]
    fn set_serialized<S: Serialize>(&mut self, query: &str, value: S) -> Result<Option<Value>> {
        let value = Value::try_from(value).map_err(Error::TomlSerialize)?;
        self.set(query, value)
    }
}

impl TomlValueSetExt for Value {
    fn set_with_seperator(
        &mut self,
        query: &str,
        sep: char,
        value: Value,
    ) -> Result<Option<Value>> {
        use crate::resolver::mut_resolver::resolve;

        let mut tokens = tokenize_with_seperator(query, sep)?;
        let last = tokens.pop_last();

        let val = resolve(self, &tokens, true)?.unwrap(); // safe because of resolve() guarantees
        let last = last.unwrap_or_else(|| Box::new(tokens));

        match *last {
            Token::Identifier { ident, .. } => match val {
                &mut Value::Table(ref mut t) => Ok(t.insert(ident, value)),
                &mut Value::Array(_) => Err(Error::NoIdentifierInArray(ident)),
                _ => Err(Error::QueryingValueAsTable(ident)),
            },

            Token::Index { idx, .. } => match val {
                &mut Value::Array(ref mut a) => {
                    if a.len() > idx {
                        let result = a.swap_remove(idx);
                        a.insert(idx, value);
                        Ok(Some(result))
                    } else {
                        a.push(value);
                        Ok(None)
                    }
                }
                &mut Value::Table(_) => Err(Error::NoIndexInTable(idx)),
                _ => Err(Error::QueryingValueAsArray(idx)),
            },
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use toml::from_str as toml_from_str;
    use toml::Value;

    #[test]
    fn test_set_with_seperator_into_table() {
        let mut toml: Value = toml_from_str(
            r#"
        [table]
        a = 0
        "#,
        )
        .unwrap();

        let res = toml.set_with_seperator(&String::from("table.a"), '.', Value::Integer(1));

        assert!(res.is_ok());

        let res = res.unwrap();
        assert!(res.is_some());
        let res = res.unwrap();
        assert!(is_match!(res, Value::Integer(0)));

        assert!(is_match!(toml, Value::Table(_)));
        match toml {
            Value::Table(ref t) => {
                assert!(!t.is_empty());

                let inner = t.get("table");
                assert!(inner.is_some());

                let inner = inner.unwrap();
                assert!(is_match!(inner, &Value::Table(_)));
                match inner {
                    &Value::Table(ref t) => {
                        assert!(!t.is_empty());

                        let a = t.get("a");
                        assert!(a.is_some());

                        let a = a.unwrap();
                        assert!(is_match!(a, &Value::Integer(1)));
                    }
                    _ => panic!("What just happenend?"),
                }
            }
            _ => panic!("What just happenend?"),
        }
    }

    #[test]
    fn test_set_with_seperator_into_table_key_nonexistent() {
        let mut toml: Value = toml_from_str(
            r#"
        [table]
        "#,
        )
        .unwrap();

        let res = toml.set_with_seperator(&String::from("table.a"), '.', Value::Integer(1));

        assert!(res.is_ok());
        let res = res.unwrap();

        assert!(res.is_none());

        assert!(is_match!(toml, Value::Table(_)));
        match toml {
            Value::Table(ref t) => {
                assert!(!t.is_empty());

                let inner = t.get("table");
                assert!(inner.is_some());

                let inner = inner.unwrap();
                assert!(is_match!(inner, &Value::Table(_)));
                match inner {
                    &Value::Table(ref t) => {
                        assert!(!t.is_empty());

                        let a = t.get("a");
                        assert!(a.is_some());

                        let a = a.unwrap();
                        assert!(is_match!(a, &Value::Integer(1)));
                    }
                    _ => panic!("What just happenend?"),
                }
            }
            _ => panic!("What just happenend?"),
        }
    }

    #[test]
    fn test_set_with_seperator_into_array() {
        use std::ops::Index;

        let mut toml: Value = toml_from_str(
            r#"
        array = [ 0 ]
        "#,
        )
        .unwrap();

        let res = toml.set_with_seperator(&String::from("array.[0]"), '.', Value::Integer(1));

        assert!(res.is_ok());

        let res = res.unwrap();
        assert!(res.is_some());
        let res = res.unwrap();
        assert!(is_match!(res, Value::Integer(0)));

        assert!(is_match!(toml, Value::Table(_)));
        match toml {
            Value::Table(ref t) => {
                assert!(!t.is_empty());

                let inner = t.get("array");
                assert!(inner.is_some());

                let inner = inner.unwrap();
                assert!(is_match!(inner, &Value::Array(_)));
                match inner {
                    &Value::Array(ref a) => {
                        assert!(!a.is_empty());
                        assert!(is_match!(a.index(0), &Value::Integer(1)));
                    }
                    _ => panic!("What just happenend?"),
                }
            }
            _ => panic!("What just happenend?"),
        }
    }

    #[test]
    fn test_set_with_seperator_into_table_index_nonexistent() {
        use std::ops::Index;

        let mut toml: Value = toml_from_str(
            r#"
        array = []
        "#,
        )
        .unwrap();

        let res = toml.set_with_seperator(&String::from("array.[0]"), '.', Value::Integer(1));

        assert!(res.is_ok());

        let res = res.unwrap();
        assert!(res.is_none());

        assert!(is_match!(toml, Value::Table(_)));
        match toml {
            Value::Table(ref t) => {
                assert!(!t.is_empty());

                let inner = t.get("array");
                assert!(inner.is_some());

                let inner = inner.unwrap();
                assert!(is_match!(inner, &Value::Array(_)));
                match inner {
                    &Value::Array(ref a) => {
                        assert!(!a.is_empty());
                        assert!(is_match!(a.index(0), &Value::Integer(1)));
                    }
                    _ => panic!("What just happenend?"),
                }
            }
            _ => panic!("What just happenend?"),
        }
    }

    #[test]
    fn test_set_with_seperator_into_nested_table() {
        let mut toml: Value = toml_from_str(
            r#"
        [a.b.c]
        d = 0
        "#,
        )
        .unwrap();

        let res = toml.set_with_seperator(&String::from("a.b.c.d"), '.', Value::Integer(1));

        assert!(res.is_ok());

        let res = res.unwrap();
        assert!(res.is_some());
        let res = res.unwrap();
        assert!(is_match!(res, Value::Integer(0)));

        assert!(is_match!(toml, Value::Table(_)));
        match toml {
            Value::Table(ref t) => {
                assert!(!t.is_empty());

                let a = t.get("a");
                assert!(a.is_some());

                let a = a.unwrap();
                assert!(is_match!(a, &Value::Table(_)));
                match a {
                    &Value::Table(ref a) => {
                        assert!(!a.is_empty());

                        let b_tab = a.get("b");
                        assert!(b_tab.is_some());

                        let b_tab = b_tab.unwrap();
                        assert!(is_match!(b_tab, &Value::Table(_)));
                        match b_tab {
                            &Value::Table(ref b) => {
                                assert!(!b.is_empty());

                                let c_tab = b.get("c");
                                assert!(c_tab.is_some());

                                let c_tab = c_tab.unwrap();
                                assert!(is_match!(c_tab, &Value::Table(_)));
                                match c_tab {
                                    &Value::Table(ref c) => {
                                        assert!(!c.is_empty());

                                        let d = c.get("d");
                                        assert!(d.is_some());

                                        let d = d.unwrap();
                                        assert!(is_match!(d, &Value::Integer(1)));
                                    }
                                    _ => panic!("What just happenend?"),
                                }
                            }
                            _ => panic!("What just happenend?"),
                        }
                    }
                    _ => panic!("What just happenend?"),
                }
            }
            _ => panic!("What just happenend?"),
        }
    }

    #[test]
    fn test_set_with_seperator_into_nonexistent_table() {
        let mut toml: Value = toml_from_str("").unwrap();

        let res = toml.set_with_seperator(&String::from("table.a"), '.', Value::Integer(1));

        assert!(res.is_err());

        let res = res.unwrap_err();
        assert!(is_match!(res, Error::IdentifierNotFoundInDocument(_)));
    }

    #[test]
    fn test_set_with_seperator_into_nonexistent_array() {
        let mut toml: Value = toml_from_str("").unwrap();

        let res = toml.set_with_seperator(&String::from("[0]"), '.', Value::Integer(1));

        assert!(res.is_err());

        let res = res.unwrap_err();
        assert!(is_match!(res, Error::NoIndexInTable(0)));
    }

    #[test]
    fn test_set_with_seperator_ident_into_ary() {
        let mut toml: Value = toml_from_str(
            r#"
        array = [ 0 ]
        "#,
        )
        .unwrap();

        let res = toml.set_with_seperator(&String::from("array.foo"), '.', Value::Integer(2));

        assert!(res.is_err());
        let res = res.unwrap_err();

        assert!(is_match!(res, Error::NoIdentifierInArray(_)));
    }

    #[test]
    fn test_set_with_seperator_index_into_table() {
        let mut toml: Value = toml_from_str(
            r#"
        foo = { bar = 1 }
        "#,
        )
        .unwrap();

        let res = toml.set_with_seperator(&String::from("foo.[0]"), '.', Value::Integer(2));

        assert!(res.is_err());
        let res = res.unwrap_err();

        assert!(is_match!(res, Error::NoIndexInTable(_)));
    }

    #[test]
    fn test_set_with_seperator_ident_into_non_structure() {
        let mut toml: Value = toml_from_str(
            r#"
        val = 0
        "#,
        )
        .unwrap();

        let res = toml.set_with_seperator(&String::from("val.foo"), '.', Value::Integer(2));

        assert!(res.is_err());
        let res = res.unwrap_err();

        assert!(is_match!(res, Error::QueryingValueAsTable(_)));
    }

    #[test]
    fn test_set_with_seperator_index_into_non_structure() {
        let mut toml: Value = toml_from_str(
            r#"
        foo = 1
        "#,
        )
        .unwrap();

        let res = toml.set_with_seperator(&String::from("foo.[0]"), '.', Value::Integer(2));

        assert!(res.is_err());
        let res = res.unwrap_err();

        assert!(is_match!(res, Error::QueryingValueAsArray(_)));
    }

    #[cfg(feature = "typed")]
    #[test]
    fn test_serialize() {
        use crate::insert::TomlValueInsertExt;
        use toml::map::Map;

        #[derive(Serialize, Deserialize, Debug)]
        struct Test {
            a: u64,
            s: String,
        }

        let mut toml = Value::Table(Map::new());
        let test = Test {
            a: 15,
            s: String::from("Helloworld"),
        };

        assert!(toml
            .insert_serialized("table.value", test)
            .unwrap()
            .is_none());

        eprintln!("{:#}", toml);

        match toml {
            Value::Table(ref tab) => match tab.get("table").unwrap() {
                &Value::Table(ref inner) => match inner.get("value").unwrap() {
                    &Value::Table(ref data) => {
                        assert!(is_match!(data.get("a").unwrap(), &Value::Integer(15)));
                        match data.get("s").unwrap() {
                            &Value::String(ref s) => assert_eq!(s, "Helloworld"),
                            _ => assert!(false),
                        };
                    }
                    _ => assert!(false),
                },
                _ => assert!(false),
            },
            _ => assert!(false),
        }
    }

}
