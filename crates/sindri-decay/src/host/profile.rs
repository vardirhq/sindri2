//! Typed reads from reusable authored profiles.

use decay_ir::Path;
use decay_runtime::{RuntimeError, Value};
use serde_json::Value as Json;
use sindri_core::ProfileDocument;

use crate::surface::ProfileCall;

use super::WorldHost;

impl WorldHost<'_> {
    pub(super) fn profile_call(
        &self,
        call: ProfileCall,
        path: &Path,
        args: &[Value],
    ) -> Result<Value, RuntimeError> {
        let profile = self.profile_argument(path, args)?;
        Ok(match call {
            ProfileCall::Name => Value::String(profile.name.clone()),
            ProfileCall::Kind => Value::String(profile.profile_type.clone()),
            ProfileCall::Number => Value::Number(scalar(
                profile,
                path,
                args,
                Json::as_f64,
                2,
                "a numeric fallback",
            )?),
            ProfileCall::Text => Value::String(scalar_text(profile, path, args)?),
            ProfileCall::Flag => Value::Bool(scalar(
                profile,
                path,
                args,
                Json::as_bool,
                2,
                "a truth fallback",
            )?),
            ProfileCall::Count => Value::Number(count(profile, path, args)?),
            ProfileCall::NumberAt => Value::Number(item(
                profile,
                path,
                args,
                Json::as_f64,
                4,
                "a numeric fallback",
            )?),
            ProfileCall::TextAt => Value::String(item_text(profile, path, args)?),
            ProfileCall::FlagAt => Value::Bool(item(
                profile,
                path,
                args,
                Json::as_bool,
                4,
                "a truth fallback",
            )?),
        })
    }

    fn profile_argument<'a>(
        &'a self,
        path: &Path,
        args: &[Value],
    ) -> Result<&'a ProfileDocument, RuntimeError> {
        let Some(Value::String(id)) = args.first() else {
            return Err(RuntimeError::Host(format!(
                "{} takes an authored Profile",
                path.dotted()
            )));
        };
        self.profiles.get(id).ok_or_else(|| {
            RuntimeError::Host(format!("{} cannot find profile '{id}'", path.dotted()))
        })
    }
}

fn count(profile: &ProfileDocument, path: &Path, args: &[Value]) -> Result<f64, RuntimeError> {
    let collection = text(path, args, 1, "a collection name")?;
    let count = profile.count(collection);
    // Authored profile collections are bounded by the asset itself, nowhere near f64's integer limit.
    #[allow(clippy::cast_precision_loss)]
    let count = count as f64;
    Ok(count)
}

fn scalar<'a, T: Copy>(
    profile: &'a ProfileDocument,
    path: &Path,
    args: &'a [Value],
    read: impl Fn(&'a Json) -> Option<T>,
    fallback: usize,
    fallback_name: &str,
) -> Result<T, RuntimeError>
where
    Value: Fallback<T>,
{
    let key = text(path, args, 1, "a value name")?;
    read_fallback(path, args, fallback, fallback_name, |fallback| {
        profile.value(key).and_then(&read).unwrap_or(fallback)
    })
}

fn item<'a, T: Copy>(
    profile: &'a ProfileDocument,
    path: &Path,
    args: &'a [Value],
    read: impl Fn(&'a Json) -> Option<T>,
    fallback: usize,
    fallback_name: &str,
) -> Result<T, RuntimeError>
where
    Value: Fallback<T>,
{
    let collection = text(path, args, 1, "a collection name")?;
    let index = index(path, args, 2)?;
    let key = text(path, args, 3, "a value name")?;
    read_fallback(path, args, fallback, fallback_name, |fallback| {
        profile
            .value_at(collection, index, key)
            .and_then(&read)
            .unwrap_or(fallback)
    })
}

fn scalar_text(
    profile: &ProfileDocument,
    path: &Path,
    args: &[Value],
) -> Result<String, RuntimeError> {
    let key = text(path, args, 1, "a value name")?;
    let fallback = text(path, args, 2, "a text fallback")?;
    Ok(profile
        .value(key)
        .and_then(Json::as_str)
        .unwrap_or(fallback)
        .to_owned())
}

fn item_text(
    profile: &ProfileDocument,
    path: &Path,
    args: &[Value],
) -> Result<String, RuntimeError> {
    let collection = text(path, args, 1, "a collection name")?;
    let index = index(path, args, 2)?;
    let key = text(path, args, 3, "a value name")?;
    let fallback = text(path, args, 4, "a text fallback")?;
    Ok(profile
        .value_at(collection, index, key)
        .and_then(Json::as_str)
        .unwrap_or(fallback)
        .to_owned())
}

fn read_fallback<T: Copy>(
    path: &Path,
    args: &[Value],
    at: usize,
    name: &str,
    read: impl FnOnce(T) -> T,
) -> Result<T, RuntimeError>
where
    Value: Fallback<T>,
{
    let fallback = args
        .get(at)
        .and_then(<Value as Fallback<T>>::fallback)
        .ok_or_else(|| RuntimeError::Host(format!("{} takes {name}", path.dotted())))?;
    Ok(read(fallback))
}

trait Fallback<T> {
    fn fallback(&self) -> Option<T>;
}

impl Fallback<f64> for Value {
    fn fallback(&self) -> Option<f64> {
        if let Self::Number(value) = self {
            Some(*value)
        } else {
            None
        }
    }
}

impl Fallback<bool> for Value {
    fn fallback(&self) -> Option<bool> {
        if let Self::Bool(value) = self {
            Some(*value)
        } else {
            None
        }
    }
}

fn text<'a>(
    path: &Path,
    args: &'a [Value],
    at: usize,
    name: &str,
) -> Result<&'a str, RuntimeError> {
    match args.get(at) {
        Some(Value::String(value)) => Ok(value),
        _ => Err(RuntimeError::Host(format!(
            "{} takes {name}",
            path.dotted()
        ))),
    }
}

fn index(path: &Path, args: &[Value], at: usize) -> Result<usize, RuntimeError> {
    let Some(Value::Number(value)) = args.get(at) else {
        return Err(RuntimeError::Host(format!(
            "{} takes an index",
            path.dotted()
        )));
    };
    if !value.is_finite() || *value < 0.0 || value.fract() != 0.0 {
        return Err(RuntimeError::Host(format!(
            "{} takes a whole non-negative index",
            path.dotted()
        )));
    }
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    Ok(*value as usize)
}
