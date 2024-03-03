use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs::File,
    io::Read,
    path::{self, Path},
    rc::Rc,
};

mod sdk;
pub use sdk::*;
mod project;
pub use project::*;
mod component;
pub use component::*;
mod definitions;
pub use definitions::*;


trait Satisfied {
    fn satisfied(&self, features: &BTreeSet<String>) -> bool;
}

macro_rules! impl_satisfied_for {
    ($t:ty) => {
        impl Satisfied for $t {
            fn satisfied(&self, features: &BTreeSet<String>) -> bool {
                if let Some(ref cond) = self.condition {
                    if !cond.iter().all(|c| features.contains(c)) {
                        return false;
                    }
                }

                if let Some(ref unless) = self.unless {
                    if unless.iter().any(|u| features.contains(u)) {
                        return false;
                    }
                }

                true
            }
        }
    };
}

impl_satisfied_for!(SourceFile);
impl_satisfied_for!(IncludeEntry);
impl_satisfied_for!(HeaderFile);
impl_satisfied_for!(Define);
impl_satisfied_for!(TemplateFile);
impl_satisfied_for!(TemplateContribution);
impl_satisfied_for!(SystemLibrary);
impl_satisfied_for!(SDKLibrary);

impl Satisfied for Library {
    fn satisfied(&self, features: &BTreeSet<String>) -> bool {
        match self {
            Library::System(s) => s.satisfied(features),
            Library::SDK(s) => s.satisfied(features),
        }
    }
}

impl Satisfied for Feature {
    fn satisfied(&self, features: &BTreeSet<String>) -> bool {
        if let Some(ref cond) = self.condition {
            return cond.iter().all(|c| features.contains(c));
        }
        true
    }
}

impl Satisfied for Require {
    fn satisfied(&self, features: &BTreeSet<String>) -> bool {
        if let Some(ref cond) = self.condition {
            return cond.iter().all(|c| features.contains(c));
        }
        true
    }
}

impl Satisfied for Conflict {
    fn satisfied(&self, features: &BTreeSet<String>) -> bool {
        if let Some(ref cond) = self.condition {
            return cond.iter().all(|c| features.contains(c));
        }
        true
    }
}

impl Satisfied for Recommendation {
    fn satisfied(&self, features: &BTreeSet<String>) -> bool {
        if let Some(ref cond) = self.condition {
            return cond.iter().all(|c| features.contains(c));
        }
        true
    }
}

// fn relative_to(path: &String, root_path: &Option<String>) -> String {
//     if let Some(root) = root_path {
//         path::PathBuf::from(root)
//             .join(path)
//             .to_string_lossy()
//             .to_string()
//     } else {
//         path.clone()
//     }
// }

// trait RelativeTo {
//     fn relative_to(&self, root_path: &Option<String>) -> Self;
// }

// macro_rules! impl_relative_to_for {
//     ($t:ty) => {
//         impl RelativeTo for $t {
//             fn relative_to(&self, root_path: &Option<String>) -> Self {
//                 Self {
//                     path: relative_to(&self.path, &root_path),
//                     ..self.clone()
//                 }
//             }
//         }
//     };
// }

// impl_relative_to_for!(SourceFile);
// impl_relative_to_for!(IncludeEntry);
// impl_relative_to_for!(HeaderFile);
// impl_relative_to_for!(TemplateFile);
