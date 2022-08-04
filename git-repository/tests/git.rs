#[cfg(not(feature = "regex"))]
mod util;
#[cfg(not(feature = "regex"))]
use util::*;

#[cfg(not(feature = "regex"))]
mod commit;
#[cfg(not(feature = "regex"))]
mod id;
#[cfg(not(feature = "regex"))]
mod init;
#[cfg(not(feature = "regex"))]
mod object;
#[cfg(not(feature = "regex"))]
mod reference;
#[cfg(not(feature = "regex"))]
mod repository;
#[cfg(not(feature = "regex"))]
mod revision;
