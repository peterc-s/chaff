//! Stores the main dataset format used by Chaff.

use std::{collections::HashMap, path::Path};

use chaff_capture::trace::Trace;

use crate::errors::DatasetError;

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

    /// Dumps the dataset in Chaff trace format into a directory of separate trace files with naming
    /// `<class_name>-<instance_num>`. Will overwrite anything with the same name.
    ///
    /// # Errors
    ///
    /// Can fail if the given path is not a directory or if serialising an individual trace fails.
    /// ([`Trace::serialise`]).
    pub fn dump_to(&self, path: &Path) -> Result<(), DatasetError> {
        if !path.is_dir() {
            return Err(DatasetError::NotADirectory(path.to_path_buf()));
        }

        for (class, traces) in &self.data {
            for (instance, trace) in traces.iter().enumerate() {
                let path = path.join(format!("{class}-{instance}"));
                trace.serialise(&path)?;
            }
        }

        Ok(())
    }
}

/// A builder struct for [`Dataset`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatasetBuilder<'a> {
    pub(crate) data: HashMap<&'a str, Vec<Trace>>,
    pub(crate) pad_to: usize,
}

impl<'a> DatasetBuilder<'a> {
    /// Create a new [`DatasetBuilder`] with an empty dataset.
    #[must_use]
    pub fn new(pad_to: usize) -> Self {
        Self {
            data: HashMap::new(),
            pad_to,
        }
    }

    /// Pushes the trace to the given class.
    pub fn push_to_class(&mut self, class: &'a str, trace: Trace) {
        let class = self.data.entry(class).or_default();
        class.push(trace);
    }

    /// Extends the given class with the given [`Trace`] iterator.
    pub fn extend_class<T: IntoIterator<Item = Trace>>(&mut self, class: &'a str, iter: T) {
        let class = self.data.entry(class).or_default();
        class.extend(iter);
    }

    /// Set the `pad_to` of the dataset builder.
    pub fn set_pad_to(&mut self, pad_to: usize) {
        self.pad_to = pad_to;
    }

    /// Build the [`Dataset`].
    #[must_use]
    pub fn build(self) -> Dataset {
        let data: HashMap<String, Box<[Trace]>> = self
            .data
            .into_iter()
            .map(|(class, traces)| (class.to_string(), traces.into_boxed_slice()))
            .collect();

        Dataset {
            data,
            pad_to: self.pad_to,
        }
    }
}

#[cfg(test)]
#[expect(clippy::unwrap_used)]
mod tests {
    use std::{collections::HashMap, fs, path::PathBuf};

    use chaff_capture::trace::{Direction, Trace};

    use crate::{dataset::DatasetBuilder, errors::DatasetError};

    use super::Dataset;

    fn temp_dir(name: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "chaff-tests-{}-{}",
            name,
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

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

    #[test]
    fn test_dump_to_errors_when_not_a_directory() {
        let dir = temp_dir("test_dump_to_errors_when_not_a_directory");

        let file_path = dir.as_path().join("not_a_dir");
        fs::write(&file_path, b"hello").unwrap();
        assert!(file_path.is_file());

        let mut builder = DatasetBuilder::new(10);
        builder.push_to_class("A", make_trace(Direction::Send));
        let dataset = builder.build();

        let err = dataset.dump_to(&file_path).unwrap_err();

        match err {
            DatasetError::NotADirectory(p) => assert_eq!(p, file_path),
            other => panic!("unexpected result: {other}"),
        }
    }

    #[test]
    fn test_dump_to_writes_expected_trace_files() {
        let out_dir = temp_dir("test_dump_to_writes_expected_trace_files");

        let mut builder = DatasetBuilder::new(10);
        builder.push_to_class("A", make_trace(Direction::Send));
        builder.push_to_class("A", make_trace(Direction::Receive));
        builder.push_to_class("B", make_trace(Direction::Send));
        let dataset = builder.build();

        dataset.dump_to(&out_dir).unwrap();

        assert!(out_dir.as_path().join("A-0").exists());
        assert!(out_dir.as_path().join("A-1").exists());
        assert!(out_dir.as_path().join("B-0").exists());
    }

    #[test]
    fn test_dataset_builder_methods_and_build() {
        let mut builder = DatasetBuilder::new(123);
        assert_eq!(builder.pad_to, 123);
        assert!(builder.data.is_empty());

        builder.push_to_class("A", make_trace(Direction::Send));
        builder.push_to_class("A", make_trace(Direction::Receive));
        assert_eq!(builder.data.get("A").unwrap().len(), 2);

        builder.extend_class(
            "B",
            vec![make_trace(Direction::Send), make_trace(Direction::Send)],
        );
        assert_eq!(builder.data.get("B").unwrap().len(), 2);

        builder.set_pad_to(999);
        assert_eq!(builder.pad_to, 999);

        let dataset = builder.build();
        assert_eq!(dataset.get_pad_to(), 999);

        let a = "A".to_string();
        assert_eq!(dataset.get_class(&a).unwrap().len(), 2);

        let b = "B".to_string();
        assert_eq!(dataset.get_class(&b).unwrap().len(), 2);
    }
}
