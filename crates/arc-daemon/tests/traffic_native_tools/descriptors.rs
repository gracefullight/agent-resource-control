//! Fixed seven native forms and their existing same-class Cargo peers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    GoBuild,
    GoTest,
    GoVet,
    SwiftBuild,
    SwiftTest,
    CmakeBuild,
    Ctest,
    CargoBuild,
    CargoTest,
    CargoClippy,
}
pub const NATIVE_FORMS: [Form; 7] = [
    Form::GoBuild,
    Form::GoTest,
    Form::GoVet,
    Form::SwiftBuild,
    Form::SwiftTest,
    Form::CmakeBuild,
    Form::Ctest,
];
impl Form {
    pub fn id(self) -> &'static str {
        match self {
            Self::GoBuild => "go-build",
            Self::GoTest => "go-test",
            Self::GoVet => "go-vet",
            Self::SwiftBuild => "swift-build",
            Self::SwiftTest => "swift-test",
            Self::CmakeBuild => "cmake-build",
            Self::Ctest => "ctest",
            Self::CargoBuild => "cargo-build",
            Self::CargoTest => "cargo-test",
            Self::CargoClippy => "cargo-clippy",
        }
    }
    pub fn is_native(self) -> bool {
        NATIVE_FORMS.contains(&self)
    }
    pub fn class(self) -> &'static str {
        match self {
            Self::GoBuild | Self::SwiftBuild | Self::CmakeBuild | Self::CargoBuild => "BUILD",
            Self::GoTest | Self::SwiftTest | Self::Ctest | Self::CargoTest => "TEST",
            Self::GoVet | Self::CargoClippy => "LINT",
        }
    }
    pub fn reason(self) -> &'static str {
        match self {
            Self::GoBuild => "GO_BUILD",
            Self::GoTest => "GO_TEST",
            Self::GoVet => "GO_VET",
            Self::SwiftBuild => "SWIFT_BUILD",
            Self::SwiftTest => "SWIFT_TEST",
            Self::CmakeBuild => "CMAKE_BUILD",
            Self::Ctest => "CTEST",
            Self::CargoBuild => "CARGO_BUILD",
            Self::CargoTest => "CARGO_TEST",
            Self::CargoClippy => "CARGO_CLIPPY",
        }
    }
    pub fn argv(self) -> &'static [&'static str] {
        match self {
            Self::GoBuild => &["go", "build"],
            Self::GoTest => &["go", "test"],
            Self::GoVet => &["go", "vet"],
            Self::SwiftBuild => &["swift", "build"],
            Self::SwiftTest => &["swift", "test"],
            Self::CmakeBuild => &["cmake", "--build", "build"],
            Self::Ctest => &["ctest", "--test-dir", "build"],
            Self::CargoBuild => &["cargo", "build", "--offline"],
            Self::CargoTest => &["cargo", "test", "--offline", "--", "--nocapture"],
            Self::CargoClippy => &["cargo", "clippy", "--offline"],
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Pair {
    pub id: &'static str,
    pub first: Form,
    pub second: Form,
}
pub const ORDERED_PAIRS: [Pair; 14] = [
    Pair {
        id: "go-build_then_cargo-build",
        first: Form::GoBuild,
        second: Form::CargoBuild,
    },
    Pair {
        id: "cargo-build_then_go-build",
        first: Form::CargoBuild,
        second: Form::GoBuild,
    },
    Pair {
        id: "go-test_then_cargo-test",
        first: Form::GoTest,
        second: Form::CargoTest,
    },
    Pair {
        id: "cargo-test_then_go-test",
        first: Form::CargoTest,
        second: Form::GoTest,
    },
    Pair {
        id: "go-vet_then_cargo-clippy",
        first: Form::GoVet,
        second: Form::CargoClippy,
    },
    Pair {
        id: "cargo-clippy_then_go-vet",
        first: Form::CargoClippy,
        second: Form::GoVet,
    },
    Pair {
        id: "swift-build_then_cargo-build",
        first: Form::SwiftBuild,
        second: Form::CargoBuild,
    },
    Pair {
        id: "cargo-build_then_swift-build",
        first: Form::CargoBuild,
        second: Form::SwiftBuild,
    },
    Pair {
        id: "swift-test_then_cargo-test",
        first: Form::SwiftTest,
        second: Form::CargoTest,
    },
    Pair {
        id: "cargo-test_then_swift-test",
        first: Form::CargoTest,
        second: Form::SwiftTest,
    },
    Pair {
        id: "cmake-build_then_cargo-build",
        first: Form::CmakeBuild,
        second: Form::CargoBuild,
    },
    Pair {
        id: "cargo-build_then_cmake-build",
        first: Form::CargoBuild,
        second: Form::CmakeBuild,
    },
    Pair {
        id: "ctest_then_cargo-test",
        first: Form::Ctest,
        second: Form::CargoTest,
    },
    Pair {
        id: "cargo-test_then_ctest",
        first: Form::CargoTest,
        second: Form::Ctest,
    },
];
