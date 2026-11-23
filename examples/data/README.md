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

`london-grid-1km.csv` holds, for each 1 km British National Grid square at
least half inside Greater London (1,597 squares), the share inside London,
the mean PTAL 2023 access index of its 100 m cells, the share in Flood
Zone 2 or 3, the LAEI 2019 modelled annual mean NO2, and the share that is
public green space. `scripts/london_grid_1km.py` builds it and
`scripts/london_suitability_deck.py` turns it into
`decks/london-suitability.sym`, whose header lists the sources and
licences.

`london-msoa-prices.csv` holds the median price paid and the number of
sales, for all dwellings and for each of detached, semi-detached, terraced
houses and flats (a price is blank where ONS suppresses it, below five
sales), in the year ending March 2023 for the 963 London
MSOAs whose 2011 boundaries were unchanged in 2021, each placed at the
centroid of its 2021 boundary (British National Grid metres). Sources: ONS
House price statistics for small areas, HPSSA datasets 1 and 2 (table 1a),
release of 20 September 2023, and the ONS Open Geography Portal's MSOA
2011 to 2021 exact-fit lookup and MSOA (December 2021) BSC boundaries.
Source: Office for National Statistics licensed under the Open Government
Licence v3.0. Contains HM Land Registry data © Crown copyright and
database right 2023. Contains OS data © Crown copyright and database
right 2023. `scripts/london_msoa_prices.py` builds it and
`scripts/london_regression_deck.py` fits the regression and writes
`decks/london-attractiveness.sym`.

`london-no2-2025.sym` is the finished deck from
[Mapping your own data](../../docs/your-own-data.md).

`dublin-ed-population.csv` holds the usually resident population (Census
2022) and area (square kilometres, from the boundary) of the 322 CSO
Electoral Divisions in County Dublin, with each division's zone reference
in `decks/dublin-dots.sym`, its ED_ID_STR code, name and local authority.
Source: Central Statistics Office (CSO) Census Hub, 'Census 2022 - Table
1.2 - Population by sex and marital status by Electoral Divisions', with
boundaries 'CSO Electoral Divisions 2022' from Tailte Éireann, licensed
under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/).
`scripts/dublin_dots_deck.py` builds it, and the dot map
`decks/dublin-dots.sym`, from the Census Hub and EPA river network
downloads named in its docstring.
