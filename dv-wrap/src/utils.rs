use std::{borrow::Cow, collections::HashMap};

pub fn var_replace<'a>(s: &'a str, vars: &HashMap<String, String>) -> Option<Cow<'a, str>> {
    let mut result = String::new();
    let mut iter = s.chars().peekable();
    while let Some(c) = iter.next() {
        if c == '$' && iter.peek() == Some(&'{') {
            iter.next(); // skip '{'
            let var_name: String = iter.by_ref().take_while(|&c| c != '}').collect();
            result.push_str(vars.get(&var_name)?);
        } else {
            result.push(c);
        }
    }
    Some(result.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_var_replace() {
        let vars = HashMap::from([
            ("USER".to_string(), "testuser".to_string()),
            ("HOME".to_string(), "/home/testuser".to_string()),
        ]);
        assert_eq!(
            var_replace("Hello ${USER}, your home is ${HOME}.", &vars).unwrap(),
            "Hello testuser, your home is /home/testuser."
        );
        assert_eq!(
            var_replace("No variable here.", &vars).unwrap(),
            "No variable here."
        );
        assert_eq!(var_replace("Undefined ${VAR}.", &vars), None);
    }
}
