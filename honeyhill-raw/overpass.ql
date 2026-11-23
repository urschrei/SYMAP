[out:json][timeout:180];
(
  way["highway"~"^(motorway|trunk|primary|secondary|tertiary|unclassified|residential|motorway_link|trunk_link|primary_link|secondary_link|tertiary_link|track)$"](42.75,-72.45,42.98,-72.15);
  way["waterway"~"^(river|stream)$"](42.75,-72.45,42.98,-72.15);
  relation["waterway"~"^(river|stream)$"](42.75,-72.45,42.98,-72.15);
  way["natural"="water"](42.75,-72.45,42.98,-72.15);
  relation["natural"="water"](42.75,-72.45,42.98,-72.15);
  way["landuse"="reservoir"](42.75,-72.45,42.98,-72.15);
  relation["landuse"="reservoir"](42.75,-72.45,42.98,-72.15);
  way["leisure"~"^(park|nature_reserve)$"](42.75,-72.45,42.98,-72.15);
  relation["leisure"~"^(park|nature_reserve)$"](42.75,-72.45,42.98,-72.15);
  way["boundary"~"^(protected_area|national_park)$"](42.75,-72.45,42.98,-72.15);
  relation["boundary"~"^(protected_area|national_park)$"](42.75,-72.45,42.98,-72.15);
  way["landuse"~"^(recreation_ground|conservation)$"](42.75,-72.45,42.98,-72.15);
  node["place"~"^(city|town|village|hamlet)$"](42.75,-72.45,42.98,-72.15);
);
out geom;
