# Maps

```rust,ignore
use dataviz::plot::geo::Projection;
ax.map(Projection::Robinson);                                  // or Mercator, NaturalEarth, …
ax.choropleth([("DEU", 83.2), ("TUR", 85.3), ("Brazil", 203.1)]).colorbar("M");
ax.geo_line([28.98, -74.0], [41.01, 40.7]);                    // great circle
ax.geo_scatter(&lon, &lat);
ax.geo_flows(&flows, &names);                                   // flow map
ax.geo_hexbin(&lon, &lat);                                      // density
ax.region_map("TUR", [("TR-34", 15.7), ("Ankara", 5.8)]);       // feature "regions"
ax.basemap((28.9, 29.1), (40.98, 41.1), 12, tile_dir("tiles"), "© OpenStreetMap contributors");
```

The world map (Natural Earth 1:110m) and, with the `regions` feature, the
states and provinces of every country (1:10m) are built in, with ISO codes,
names, population and GDP; `geo::read_geojson` reads other shapes. Tiles
come from any source you give (`tile_dir` reads a folder); credit the
provider.
