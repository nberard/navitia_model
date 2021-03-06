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

use model::Collections;
use objects;
use std::io::Read;
use Result;

extern crate minidom;
extern crate serde_json;
use self::minidom::Element;
use failure::ResultExt;

// type RoutePointId = String;
// type StopPointId = String;
// type RoutePointMapping = HashMap<RoutePointId, StopPointId>;
// type RouteLineMap = HashMap<String, String>;

#[derive(Default)]
struct NetexContext {
    namespace: String,
    first_operator_id: String,
    // network_id: String,
    // routepoint_mapping: RoutePointMapping,
    // route_line_map: RouteLineMap,
    // route_mode_map: HashMap<String, String>,
    // journeypattern_route_map: HashMap<String, String>,
}

#[derive(Default)]
pub struct NetexReader {
    context: NetexContext,
    pub collections: Collections,
}
impl NetexReader {
    pub fn read_netex_file<R: Read>(&mut self, mut file: R) -> Result<()> {
        let mut file_content = "".to_string();
        file.read_to_string(&mut file_content)?;
        let root: Element = file_content.parse()?;

        self.context.namespace = root.ns().unwrap_or("".to_string());

        for frame in root
            .get_child("dataObjects", self.context.namespace.as_str())
            .ok_or_else(|| format_err!("Netex file does't contain a 'dataObjects' node"))?
            .children()
            .filter(|frame| frame.name() == "CompositeFrame")
        {
            self.read_composite_data_frame(frame).with_context(|_| {
                format!(
                    "Reading Frame id={:?}",
                    frame.attr("id").unwrap_or("undefined")
                )
            })?
        }
        Ok(())
    }

    fn read_composite_data_frame(&mut self, composite_frame: &Element) -> Result<()> {
        for frame in composite_frame
            .get_child("frames", self.context.namespace.as_str())
            .ok_or_else(|| format_err!("CompositeDataFrame does't contain a 'frames' node"))?
            .children()
        {
            match frame.name() {
                // "SiteFrame" => self.read_site_frame(&frame),
                // "ServiceFrame" => self.read_service_frame(&frame),
                // "ServiceCalendarFrame" => self.read_service_calendar_frame(&frame),
                // "TimetableFrame" => self.read_time_table_frame(&frame),
                "ResourceFrame" => self.read_resource_frame(&frame),
                _ => Ok(()),
            }?
        }
        Ok(())
    }

    fn read_resource_frame(&mut self, resource_frame: &Element) -> Result<()> {
        // a ResourceFrame contains 0..1 organisations or 0..1 groupsOfOperators
        // (other objects don't seem to be relevant for Navitia)
        // for the moment, only reading "organisations" until a groupsOfOperators use is encontered.

        let organisations = resource_frame.get_child("organisations", &self.context.namespace);
        match organisations {
            None => Ok(()),
            Some(orgs) => self.read_organisations(&orgs),
        }
    }

    fn read_organisations(&mut self, organisations: &Element) -> Result<()> {
        let companies = organisations
            .children()
            .filter(|node| node.name() == "Operator")
            .map(|node| match node.attr("id") {
                Some(id) => Ok(objects::Company {
                    id: id.to_string(),
                    name: node
                        .get_child("Name", &self.context.namespace)
                        .map_or("".to_string(), |n| n.text().to_string()),
                    ..Default::default()
                }),
                _ => bail!("An 'Operator' node doesn't have an 'id' property."),
            })
            .collect::<Result<Vec<_>>>()?;
        if !companies.is_empty() {
            self.context.first_operator_id = companies[0].id.to_string();
            for c in companies {
                if self.collections.companies.get_idx(&c.id).is_none() {
                    self.collections.companies.push(c)?;
                }
            }
        } else {
            self.context.first_operator_id = "default_company".to_string();
            if self
                .collections
                .companies
                .get_idx(&self.context.first_operator_id)
                .is_none()
            {
                self.collections
                    .companies
                    .push(objects::Company::default())?;
            };
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    extern crate minidom;
    use self::minidom::Element;

    #[test]
    fn test_read_organisations_empty() {
        let mut netex_reader = super::NetexReader::default();
        let organisations = Element::bare("organisations");
        netex_reader.read_organisations(&organisations).unwrap();
        assert_eq!(netex_reader.collections.companies.len(), 1);
        let company = netex_reader.collections.companies.iter().next().unwrap().1;
        assert_eq!(company.id, "default_company");
    }

    #[test]
    fn test_read_organisations_normal() {
        let mut netex_reader = super::NetexReader::default();
        let mut organisations = Element::builder("organisations").ns("").build();
        let operator: Element = r#"<Operator version="1" id="RATP_PIVI:Company:100">
							<CompanyNumber>100</CompanyNumber>
							<Name>RATP</Name>
						</Operator>"#.parse()
            .unwrap();
        organisations.append_child(operator);

        netex_reader.read_organisations(&organisations).unwrap();
        assert_eq!(netex_reader.collections.companies.len(), 1);
        let company = netex_reader.collections.companies.iter().next().unwrap().1;
        assert_eq!(company.id, "RATP_PIVI:Company:100");
    }

    #[test]
    fn test_read_organisations_no_name() {
        let mut netex_reader = super::NetexReader::default();
        let mut organisations = Element::builder("organisations").ns("").build();
        let operator: Element = r#"<Operator version="1" id="RATP_PIVI:Company:100">
							<CompanyNumber>100</CompanyNumber>
						</Operator>"#.parse()
            .unwrap();
        organisations.append_child(operator);

        netex_reader.read_organisations(&organisations).unwrap();
        assert_eq!(netex_reader.collections.companies.len(), 1);
        let company = netex_reader.collections.companies.iter().next().unwrap().1;
        assert_eq!(company.id, "RATP_PIVI:Company:100");
        assert_eq!(company.name, "");
    }

    #[test]
    fn test_read_organisations_no_id() {
        let mut netex_reader = super::NetexReader::default();
        let mut organisations = Element::builder("organisations").ns("").build();
        let operator: Element = r#"<Operator version="1" identifier="RATP_PIVI:Company:100">
							<CompanyNumber>100</CompanyNumber>
						</Operator>"#.parse()
            .unwrap();
        organisations.append_child(operator);

        assert!(netex_reader.read_organisations(&organisations).is_err());
        assert_eq!(netex_reader.collections.companies.len(), 0);
    }
}
