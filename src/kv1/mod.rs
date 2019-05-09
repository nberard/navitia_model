// Copyright 2017 Kisio Digital and/or its affiliates.
//
// This program is free software: you can redistribute it and/or
// modify it under the terms of the GNU General Public License as
// published by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful, but
// WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the GNU
// General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see
// <http://www.gnu.org/licenses/>.

//! KV1 format management.

mod read;

use crate::model::{Collections, Model};
use crate::read_utils;
use crate::read_utils::add_prefix;
use crate::Result;
use std::fs::File;
use std::path::Path;

fn read<H>(file_handler: &mut H, prefix: Option<String>) -> Result<Model>
where
    for<'a> &'a mut H: read_utils::FileHandler,
{
    let mut collections = Collections::default();

    read::read_operday(file_handler, &mut collections)?;
    read::make_physical_and_commercial_modes(&mut collections);
    read::read_usrstop_point(file_handler, &mut collections)?;
    read::read_usrstar(file_handler, &mut collections)?;
    read::read_jopa_pujopass_line(file_handler, &mut collections)?;
    read::read_notice_ntcassgn(file_handler, &mut collections)?;

    //add prefixes
    if let Some(prefix) = prefix {
        add_prefix(prefix, &mut collections)?;
    }

    Ok(Model::new(collections)?)
}

/// Imports a `Model` from the KV1 files in the `path` directory.
///
/// The `prefix` argument is a string that will be prepended to every
/// identifiers, allowing to namespace the dataset. By default, no
/// prefix will be added to the identifiers.
pub fn read_from_path<P: AsRef<Path>>(p: P, prefix: Option<String>) -> Result<Model> {
    let mut file_handle = read_utils::PathFileHandler::new(p.as_ref().to_path_buf());
    read(&mut file_handle, prefix)
}

/// Imports a `Model` from a zip file containing the KV1.
///
/// The `prefix` argument is a string that will be prepended to every
/// identifiers, allowing to namespace the dataset. By default, no
/// prefix will be added to the identifiers.
pub fn read_from_zip<P: AsRef<Path>>(p: P, prefix: Option<String>) -> Result<Model> {
    let reader = File::open(p.as_ref())?;
    let mut file_handle = read_utils::ZipHandler::new(reader, p)?;
    read(&mut file_handle, prefix)
}

/// Imports a `Model` from a url hosting a zip file containing the KV1.
///
/// The `prefix` argument is a string that will be prepended to every
/// identifiers, allowing to namespace the dataset. By default, no
/// prefix will be added to the identifiers.
pub fn read_from_url(url: &str, prefix: Option<String>) -> Result<Model> {
    let reader = read_utils::read_url(&url)?;
    let mut file_handle = read_utils::ZipHandler::new(reader, &url)?;
    read(&mut file_handle, prefix)
}