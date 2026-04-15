//! Stores the main dataset format used by Chaff.

use std::collections::HashMap;

use chaff_capture::trace::Trace;

/// The main dataset type for [`crate`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dataset {
    /// The actual dataset.
    pub(crate) data: HashMap<String, Box<[Trace]>>,

    /// What length the dataset's traces should be padded to.
    pub(crate) pad_to: usize,
}

impl Dataset {
    /// Returns a [`Vec`] of class names in the dataset.
    #[must_use]
    pub fn classes(&self) -> Vec<&String> {
        self.data.keys().collect()
    }

    /// Returns the length to which a dataset will be padded to with `0`s.
    #[must_use]
    pub fn get_pad_to(&self) -> usize {
        self.pad_to
    }

    /// Returns a reference to the dataset.
    #[must_use]
    pub fn get_dataset(&self) -> &HashMap<String, Box<[Trace]>> {
        &self.data
    }

    /// Returns a reference to the traces for a given class.
    #[must_use]
    pub fn get_class(&self, class: &String) -> Option<&[Trace]> {
        self.data.get(class).map(|v| &**v)
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use std::collections::HashMap;

    use chaff_capture::trace::{Direction, Trace};

    use super::Dataset;

    fn make_trace(dir: Direction) -> Trace {
        Trace {
            directions: Box::new([dir]),
            timing_deltas: Box::new([0]),
            sizes: Box::new([1]),
        }
    }

    // Mostly for tarpaulin coverage. these are trivial methods.
    #[test]
    fn test_dataset_accessors() {
        let mut test_data: HashMap<String, Box<[Trace]>> = HashMap::new();
        test_data.insert(
            "A".to_string(),
            vec![make_trace(Direction::Send)].into_boxed_slice(),
        );
        test_data.insert(
            "B".to_string(),
            vec![make_trace(Direction::Receive), make_trace(Direction::Send)].into_boxed_slice(),
        );

        let dataset = Dataset {
            data: test_data,
            pad_to: 5000,
        };

        assert_eq!(dataset.get_pad_to(), 5000);

        let data = dataset.get_dataset();
        assert_eq!(data.len(), 2);
        assert!(data.contains_key("A"));
        assert!(data.contains_key("B"));

        let mut classes = dataset.classes();
        classes.sort();
        assert_eq!(classes, vec![&"A".to_string(), &"B".to_string()]);

        let a = "A".to_string();
        let a_traces = dataset.get_class(&a).unwrap();
        assert_eq!(a_traces.len(), 1);
        assert_eq!(a_traces[0].directions[0], Direction::Send);

        let missing = "><>".to_string();
        assert!(dataset.get_class(&missing).is_none());
    }
}
