use std::collections::BTreeSet;

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
impl_satisfied_for!(ConfigFile);
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
