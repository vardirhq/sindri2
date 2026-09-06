use decay_semantic::{ExternalSymbol, Type};

use crate::{
    environment,
    surface::{ENTITY, PROFILE, PROFILE_CALLS, PROFILES, names},
};

#[test]
fn profile_surface_uses_a_typed_asset_reference() {
    let environment = environment();
    let profile = Type::Named(PROFILE.to_owned());

    assert!(environment.get_type(PROFILE).is_some());
    assert!(environment.get_type(ENTITY).is_some());

    let profiles = environment
        .get_type(PROFILES)
        .expect("Profiles should be a described host type");
    for (name, _) in PROFILE_CALLS {
        let Some(ExternalSymbol::Function(function)) = profiles.member(name) else {
            panic!("Profiles.{name} should be a described function");
        };
        assert_eq!(
            function.params.first(),
            Some(&profile),
            "Profiles.{name} should take a Profile first"
        );
    }

    assert!(environment.globals().any(|(name, symbol)| {
        name == PROFILES && symbol == &ExternalSymbol::Value(Type::Named(PROFILES.to_owned()))
    }));
}

#[test]
fn the_namespace_names_are_stable() {
    assert_eq!(names::WORLD, "World");
    assert_eq!(names::PROFILES, "Profiles");
}
