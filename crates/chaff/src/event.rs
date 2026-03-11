// For easily working with events.
macro_rules! enum_index {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        #[repr(usize)]
        #[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub const COUNT: usize = enum_index!(@count $($variant),+);

            pub const ALL: [Self; Self::COUNT] = [
                $(Self::$variant),+
            ];

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
    Event {
        SendNormal,
        ReceiveNormal,
    }
}
