// src/non_empty_vec.rs

#[derive(Debug, PartialEq)]
pub struct NonEmptyVec<T>(Vec<T>);

impl<T> NonEmptyVec<T> {
    pub fn new(vec: Vec<T>) -> Option<Self> {
        if vec.is_empty() {
            None
        } else {
            Some(NonEmptyVec(vec))
        }
    }
    pub fn as_slice(&self) -> &[T] {
        &self.0
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_with_empty_vec_returns_none() {
        let result: Option<NonEmptyVec<String>> = NonEmptyVec::new(vec![]);
        assert!(result.is_none());
    }

    #[test]
    fn new_with_one_element_returns_some() {
        let result = NonEmptyVec::new(vec!["item".to_string()]);
        assert!(result.is_some());
    }

    #[test]
    fn new_with_multiple_elements_returns_some() {
        let result = NonEmptyVec::new(vec!["a".to_string(), "b".to_string()]);
        assert!(result.is_some());
    }

    #[test]
    fn as_slice_returns_inner_data() {
        let nev = NonEmptyVec::new(vec!["a".to_string(), "b".to_string()]).unwrap();
        assert_eq!(nev.as_slice(), &["a".to_string(), "b".to_string()]);
    }
}