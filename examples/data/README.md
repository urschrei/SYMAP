# Data for the tutorial

`london-no2-2025.csv` holds the annual mean concentration of nitrogen
dioxide (micrograms per cubic metre) in 2025 at 71 continuous monitoring
sites in and around Greater London, with each site's name, code, type,
longitude and latitude. Sites with less than 75% data capture are left out.

Source: Defra UK-AIR annual summaries for the AURN and locally managed
networks (`summary_annual_AURN_2025.rds`, `summary_annual_LMAM_2025.rds`)
and their site metadata, <https://uk-air.defra.gov.uk/>. © Crown copyright
2026 Defra via uk-air.defra.gov.uk, licensed under the
[Open Government Licence v3.0](https://www.nationalarchives.gov.uk/doc/open-government-licence/version/3/).

`thames.geojson` is the River Thames from Staines to Purfleet, one
MultiLineString in longitude and latitude, simplified to within about 60 m.
Source: OS Open Rivers, Ordnance Survey,
<https://osdatahub.os.uk/downloads/open/OpenRivers>. Contains OS data ©
Crown copyright and database right 2026, licensed under the Open
Government Licence v3.0.

`london-no2-2025.sym` is the finished deck from
[Mapping your own data](../../docs/your-own-data.md).
