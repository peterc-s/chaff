//! Stores the main dataset format used by Chaff.

use std::collections::HashMap;

use chaff_capture::trace::Trace;

/// The main dataset type for [`crate`].
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
