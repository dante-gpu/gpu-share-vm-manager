use std::error::Error;
use std::result::Result;

#[allow(dead_code)]  // for now my friends dont worry about it
pub type DynError = Box<dyn std::error::Error + Send + Sync>;
pub type DynResult<T> = Result<T, DynError>; 