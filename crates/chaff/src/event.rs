//! Chaff events.

// For easily working with events.
macro_rules! enum_index {
    (
        $(#[$enum_attr:meta])*
        $name:ident {
            $(
                $(#[$variant_attr:meta])*
                $variant:ident
            ),+ $(,)?
        }
    ) => {
        $(#[$enum_attr])*
        #[repr(usize)]
        #[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
        pub enum $name {
            $(
                $(#[$variant_attr])*
                $variant
            ),+
        }
        impl $name {
            /// Number of variants in the enum.
            pub const COUNT: usize = enum_index!(@count $($variant),+);

            /// An array of all the variants in the enum.
            pub const ALL: [Self; Self::COUNT] = [
                $(Self::$variant),+
            ];

            /// Returns the index of a variant.
            pub const fn index(self) -> usize {
                self as usize
            }
        }
    };

    // token counting dark arts...
    (@count $($tts:tt),*) => {
        // this is using the array of units len method
        // to count the "replace" of each token, which, from
        // the name rule is the variants of the enum
        // the replace rule simply replaces each variant with a unit
        // therefore, if we have two variants, this should become
        // <[()]>::len(&[(), ()]) which is 2, as it should be.
        <[()]>::len(&[$(enum_index!(@replace $tts ())),*])
    };

    (@replace $_t:tt $sub:expr) => {$sub};
}

enum_index! {
    /// Events
    Event {
        /// Normal packet sent (egress)
        SendNormal,

        /// Normal packet received (ingress)
        ReceiveNormal,
    }
}
