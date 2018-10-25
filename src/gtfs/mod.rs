// Copyright 2017-2018 Kisio Digital and/or its affiliates.
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

//! [GTFS](http://gtfs.org/) format management.

mod read;
mod write;

use collection::CollectionWithId;
use common_format;
use common_format::{manage_calendars, Availability};
use gtfs::read::EquipmentList;
use model::{Collections, Model};
use objects;
use objects::Time;
use read_utils::add_prefix;
use std::path::Path;
use utils::*;
use Result;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct Agency {
    #[serde(rename = "agency_id")]
    id: Option<String>,
    #[serde(rename = "agency_name")]
    name: String,
    #[serde(rename = "agency_url")]
    url: String,
    #[serde(rename = "agency_timezone")]
    timezone: String,
    #[serde(rename = "agency_lang")]
    lang: Option<String>,
    #[serde(rename = "agency_phone")]
    phone: Option<String>,
    #[serde(rename = "agency_email")]
    email: Option<String>,
}

impl<'a> From<&'a objects::Network> for Agency {
    fn from(obj: &objects::Network) -> Agency {
        Agency {
            id: Some(obj.id.clone()),
            name: obj.name.clone(),
            url: obj
                .url
                .clone()
                .unwrap_or_else(|| "http://www.navitia.io/".to_string()),
            timezone: obj
                .timezone
                .clone()
                .unwrap_or_else(|| "Europe/Paris".to_string()),
            lang: obj.lang.clone(),
            phone: obj.phone.clone(),
            email: None,
        }
    }
}

#[derivative(Default)]
#[derive(Derivative, Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
enum StopLocationType {
    #[derivative(Default)]
    #[serde(rename = "0")]
    StopPoint,
    #[serde(rename = "1")]
    StopArea,
    #[serde(rename = "2")]
    StopEntrace,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct Stop {
    #[serde(rename = "stop_id")]
    id: String,
    #[serde(rename = "stop_code")]
    code: Option<String>,
    #[serde(rename = "stop_name")]
    name: String,
    #[serde(default, rename = "stop_desc")]
    desc: String,
    #[serde(rename = "stop_lon")]
    lon: f64,
    #[serde(rename = "stop_lat")]
    lat: f64,
    #[serde(rename = "zone_id")]
    fare_zone_id: Option<String>,
    #[serde(rename = "stop_url")]
    url: Option<String>,
    #[serde(default, deserialize_with = "de_with_empty_default")]
    location_type: StopLocationType,
    parent_station: Option<String>,
    #[serde(rename = "stop_timezone")]
    timezone: Option<String>,
    #[serde(deserialize_with = "de_with_empty_default", default)]
    wheelchair_boarding: Availability,
}

#[derive(Derivative)]
#[derivative(Default)]
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
enum DirectionType {
    #[derivative(Default)]
    #[serde(rename = "0")]
    Forward,
    #[serde(rename = "1")]
    Backward,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
struct Trip {
    route_id: String,
    service_id: String,
    #[serde(rename = "trip_id")]
    id: String,
    #[serde(rename = "trip_headsign")]
    headsign: Option<String>,
    #[serde(rename = "trip_short_name")]
    short_name: Option<String>,
    #[serde(
        default,
        deserialize_with = "de_with_empty_default",
        rename = "direction_id"
    )]
    direction: DirectionType,
    block_id: Option<String>,
    shape_id: Option<String>,
    #[serde(deserialize_with = "de_with_empty_default", default)]
    wheelchair_accessible: Availability,
    #[serde(deserialize_with = "de_with_empty_default", default)]
    bikes_allowed: Availability,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
struct StopTime {
    trip_id: String,
    arrival_time: Time,
    departure_time: Time,
    stop_id: String,
    stop_sequence: u32,
    #[serde(deserialize_with = "de_with_empty_default", default)]
    pickup_type: u8,
    #[serde(deserialize_with = "de_with_empty_default", default)]
    drop_off_type: u8,
    local_zone_id: Option<u16>,
    stop_headsign: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Derivative, PartialEq)]
#[derivative(Default)]
enum TransferType {
    #[derivative(Default)]
    #[serde(rename = "0")]
    Recommended,
    #[serde(rename = "1")]
    Timed,
    #[serde(rename = "2")]
    WithTransferTime,
    #[serde(rename = "3")]
    NotPossible,
}

#[derive(Serialize, Deserialize, Debug, PartialEq)]
struct Transfer {
    from_stop_id: String,
    to_stop_id: String,
    #[serde(deserialize_with = "de_with_empty_default")]
    transfer_type: TransferType,
    min_transfer_time: Option<u32>,
}

impl<'a> From<&'a objects::Transfer> for Transfer {
    fn from(obj: &objects::Transfer) -> Transfer {
        Transfer {
            from_stop_id: obj.from_stop_id.clone(),
            to_stop_id: obj.to_stop_id.clone(),
            transfer_type: TransferType::WithTransferTime,
            min_transfer_time: obj.min_transfer_time,
        }
    }
}

/// Imports a `Model` from the [GTFS](http://gtfs.org/) files in the
/// `path` directory.
///
/// The `config_path` argument allows you to give a path to a file
/// containing a json representing the contributor and dataset used
/// for this GTFS. If not given, default values will be created.
///
/// The `prefix` argument is a string that will be prepended to every
/// identifiers, allowing to namespace the dataset. By default, no
/// prefix will be added to the identifiers.
pub fn read<P>(path: P, config_path: Option<P>, prefix: Option<String>) -> Result<Model>
where
    P: AsRef<Path>,
{
    let mut collections = Collections::default();
    let mut equipments = EquipmentList::default();
    let mut comments: CollectionWithId<objects::Comment> = CollectionWithId::default();

    let path = path.as_ref();

    manage_calendars(&mut collections, path)?;

    let (contributors, mut datasets) = read::read_config(config_path)?;
    read::set_dataset_validity_period(&mut datasets, &collections.calendars)?;

    collections.contributors = contributors;
    collections.datasets = datasets;

    let (networks, companies) = read::read_agency(path)?;
    collections.networks = networks;
    collections.companies = companies;
    let (stop_areas, stop_points) = read::read_stops(path, &mut comments, &mut equipments)?;
    collections.transfers = read::read_transfers(path, &stop_points)?;
    collections.stop_areas = stop_areas;
    collections.stop_points = stop_points;

    read::manage_shapes(&mut collections, path)?;

    read::read_routes(path, &mut collections)?;
    collections.equipments = CollectionWithId::new(equipments.into_equipments())?;
    collections.comments = comments;
    read::manage_stop_times(&mut collections, path)?;

    //add prefixes
    if let Some(prefix) = prefix {
        add_prefix(prefix, &mut collections)?;
    }

    Ok(Model::new(collections)?)
}

/// Exports a `Model` to [GTFS](http://gtfs.org/) files
/// in the given directory.
pub fn write<P: AsRef<Path>>(model: &Model, path: P) -> Result<()> {
    let path = path.as_ref();
    info!("Writing GTFS to {:?}", path);

    write::write_transfers(path, &model.transfers)?;
    write::write_agencies(path, &model.networks)?;
    common_format::write_calendar_dates(path, &model.calendars)?;
    write::write_stops(
        path,
        &model.stop_points,
        &model.stop_areas,
        &model.comments,
        &model.equipments,
    )?;
    write::write_trips(
        path,
        &model.vehicle_journeys,
        &model.stop_points,
        &model.routes,
        &model.trip_properties,
    )?;
    write::write_stop_times(path, &model.vehicle_journeys, &model.stop_points)?;

    Ok(())
}
