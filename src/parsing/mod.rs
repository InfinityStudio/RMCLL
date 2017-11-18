#![allow(dead_code)]

use std::rc::Rc;

#[derive(Clone)]
pub enum ParameterStrategy {
    Ignore,
    Map(Rc<Fn(String) -> String>),
}

pub struct ArgumentIterator<'a> {
    strategy: &'a ParameterStrategy,
    chars: Vec<char>,
    index: usize,
}

impl ParameterStrategy {
    pub fn ignore() -> ParameterStrategy {
        ParameterStrategy::Ignore
    }

    pub fn map<F: Fn(String) -> String + 'static>(function: F) -> ParameterStrategy {
        ParameterStrategy::Map(Rc::new(function))
    }
}

impl<'a> Iterator for ArgumentIterator<'a> {
    type Item = String;

    fn next(&mut self) -> Option<String> {
        let (index, result) = parse_whole_string(&self.chars, self.index, &self.strategy);
        self.index = index;
        result
    }
}

pub fn parse<'a>(string: &str, strategy: &'a ParameterStrategy) -> ArgumentIterator<'a> {
    ArgumentIterator { strategy, chars: string.chars().collect(), index: 0 }
}

fn parse_whole_string(chars: &Vec<char>, original_pos: usize, strategy: &ParameterStrategy) -> (usize, Option<String>) {
    let mut index = original_pos;
    let mut result: String = String::new();
    while let Some(c) = chars.get(index) { if c.is_whitespace() { index += 1; } else { break; } }
    loop {
        match chars.get(index) {
            None => return (index, Some(result)),
            Some(c) if c.is_whitespace() => return (index, Some(result)),
            Some(&'$') => match parse_dollar_parameters(chars, index, strategy) {
                (i, None) => return (i, None),
                (i, Some(string)) => {
                    result.push_str(&string);
                    index = i;
                }
            }
            Some(&'\'') => match parse_single_quote(chars, index, strategy) {
                (i, None) => return (i, None),
                (i, Some(string)) => {
                    result.push_str(&string);
                    index = i;
                }
            }
            Some(&'\"') => match parse_double_quote(chars, index, strategy) {
                (i, None) => return (i, None),
                (i, Some(string)) => {
                    result.push_str(&string);
                    index = i;
                }
            }
            Some(&'\\') => {
                index += 1;
                match chars.get(index) {
                    Some(c @ &'\n') | Some(c @ &'\r') => {
                        if let &ParameterStrategy::Ignore = strategy {
                            result.push('\\');
                            result.push(c.clone());
                        }
                        index += 1;
                    }
                    Some(c) => {
                        if let &ParameterStrategy::Ignore = strategy { result.push('\\') }
                        index += 1;
                        result.push(c.clone());
                    }
                    None => return (original_pos, None)
                }
            }
            Some(c) => {
                index += 1;
                result.push(c.clone());
            }
        }
    }
}

fn parse_single_quote(chars: &Vec<char>, pos: usize, strategy: &ParameterStrategy) -> (usize, Option<String>) {
    let mut index = pos;
    let mut result: String = String::new();
    if let &ParameterStrategy::Ignore = strategy { result.push('\'') }
    index += 1;
    loop {
        if let Some(c) = chars.get(index) {
            if c == &'\'' {
                if let &ParameterStrategy::Ignore = strategy { result.push('\'') }
                index += 1;
                return (index, Some(result));
            }
            result.push(c.clone());
            index += 1;
        } else {
            return (pos, None);
        }
    }
}

fn parse_double_quote(chars: &Vec<char>, pos: usize, strategy: &ParameterStrategy) -> (usize, Option<String>) {
    let mut index = pos;
    let mut result: String = String::new();
    if let &ParameterStrategy::Ignore = strategy { result.push('\"') }
    index += 1;
    loop {
        match chars.get(index) {
            Some(&'\\') => {
                index += 1;
                match chars.get(index) {
                    Some(c @ &'\"') | Some(c @ &'$') => {
                        if let &ParameterStrategy::Ignore = strategy { result.push('\\') }
                        index += 1;
                        result.push(c.clone());
                    }
                    Some(c @ &'\n') | Some(c @ &'\r') => {
                        if let &ParameterStrategy::Ignore = strategy {
                            result.push('\\');
                            result.push(c.clone());
                        }
                        index += 1;
                    }
                    Some(&_) => result.push('\\'),
                    None => return (pos, None)
                }
            }
            Some(&'\"') => {
                if let &ParameterStrategy::Ignore = strategy { result.push('\"') }
                index += 1;
                return (index, Some(result));
            }
            Some(&'$') => match parse_dollar_parameters(chars, index, strategy) {
                (i, None) => return (i, None),
                (i, Some(string)) => {
                    result.push_str(&string);
                    index = i;
                }
            }
            Some(c) => {
                result.push(c.clone());
                index += 1;
            }
            None => return (pos, None)
        }
    }
}

fn parse_dollar_parameters(chars: &Vec<char>, pos: usize, strategy: &ParameterStrategy) -> (usize, Option<String>) {
    match strategy {
        &ParameterStrategy::Ignore => return (pos + 1, Some("$".to_owned())),
        &ParameterStrategy::Map(ref b) => {
            let mut index = pos + 1;
            let mut result = String::new();
            loop {
                match chars.get(index) {
                    Some(&'{') => {
                        index += 1;
                        loop {
                            if let Some(c) = chars.get(index) {
                                if c == &'}' { return (index + 1, Some(b.as_ref()(result))); }
                                result.push(c.clone());
                                index += 1;
                            } else {
                                return (pos, None);
                            }
                        }
                    }
                    Some(c @ &'a' ... 'z') | Some(c @ &'A' ... 'Z') |
                    Some(c @ &'0' ... '9') | Some(c @ &'_') => {
                        result.push(c.clone());
                        index += 1;
                    }
                    _ if result.is_empty() => return (pos + 1, Some("$".to_owned())),
                    _ => return (index, Some(b.as_ref()(result)))
                }
            }
        }
    }
}
