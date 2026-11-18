//! CABIN-OSM · TRAFFIC BAKER
//!   · reads data/ndls.traffic.toml, the sourced profile (slice B1)
//!   · resolves each pool train's class, rake length and loco from the
//!     profile's [mix] and [length_m] tables
//!   · writes data/ndls.traffic, a plain line format the sim parses with no
//!     dependency, as data/ndls.yard is for the yard
//!
//! The baked file holds values only. Sources and tags stay in the `.toml`,
//! where a test checks them; a second test demands that the committed
//! `.traffic` is exactly what the `.toml` bakes to, so the two cannot drift.

/// Pool type to (class in [mix], key in [length_m], loco-hauled). Tejas,
/// Humsafar and Jan Shatabdi are counted as express: the mix names the
/// premium three only (V-27 to V-29). An EMU or MEMU rake has no loco;
/// a Passenger is loco-hauled with a MEMU-sized rake.
const TYPES: [(&str, &str, &str, bool); 12] = [
    ("EMU/MEMU", "emu_local", "emu", false),
    ("Passenger", "emu_local", "passenger", true),
    ("Superfast", "express", "superfast", true),
    ("Mail/Express", "express", "mail_express", true),
    ("Humsafar", "express", "humsafar", true),
    ("Garib Rath", "express", "garib_rath", true),
    ("Jan Shatabdi", "express", "jan_shatabdi", true),
    ("Tejas", "express", "tejas", true),
    ("Rajdhani", "rajdhani", "rajdhani", true),
    ("Shatabdi", "shatabdi", "shatabdi", true),
    ("Duronto", "duronto", "duronto", true),
    ("Vande Bharat", "vande_bharat", "vande_bharat_16", false),
];

/// Class in [mix] to its through dwell in [dwell_s]. The premium split
/// (rajdhani, shatabdi, duronto) replaces `premium`, so the shares sum to 1.
const CLASSES: [(&str, &str); 6] = [
    ("emu_local", "through_emu"),
    ("express", "through_express"),
    ("rajdhani", "through_express"),
    ("shatabdi", "through_express"),
    ("duronto", "through_express"),
    ("vande_bharat", "through_express"),
];

pub fn bake(text: &str) -> Result<String, String> {
    let doc: toml::Table = text.parse().map_err(|e| format!("ndls.traffic.toml: {e}"))?;
    let v = |section: &str, key: &str| -> Result<&toml::Value, String> {
        doc.get(section).and_then(|s| s.get(key)).and_then(|x| x.get("v")).ok_or(format!("ndls.traffic.toml: no {section}.{key}"))
    };
    let num = |section: &str, key: &str| -> Result<f64, String> {
        let x = v(section, key)?;
        x.as_float().or(x.as_integer().map(|i| i as f64)).ok_or(format!("ndls.traffic.toml: {section}.{key} is not a number"))
    };
    let ints = |section: &str, key: &str| -> Result<Vec<i64>, String> {
        v(section, key)?
            .as_array()
            .and_then(|a| a.iter().map(|x| x.as_integer()).collect::<Option<Vec<_>>>())
            .ok_or(format!("ndls.traffic.toml: {section}.{key} is not a list of whole numbers"))
    };
    let pair = |key: &str| -> Result<String, String> {
        let d = ints("dwell_s", key)?;
        match d[..] {
            [lo, hi] if 0 < lo && lo <= hi => Ok(format!("{lo} {hi}")),
            _ => Err(format!("ndls.traffic.toml: dwell_s.{key} must be [low, high]")),
        }
    };

    let mut out = String::from("# cabin traffic profile, format 1. Written by cabin-osm from data/ndls.traffic.toml; do not edit.\n# Values only: each one's source and tag are in the .toml.\n");
    // The date the ledgers were read, for the "what is real" panel.
    let written = v("meta", "written")?.as_str().filter(|d| d.len() == 10 && !d.contains(char::is_whitespace));
    out += &format!("written {}\n", written.ok_or("ndls.traffic.toml: meta.written must be a date, YYYY-MM-DD")?);
    let hourly = ints("volume", "hourly_weights")?;
    if hourly.len() != 24 || hourly.iter().any(|&h| h < 0) {
        return Err("ndls.traffic.toml: volume.hourly_weights needs 24 counts".into());
    }
    out += &format!("hourly {}\n", hourly.iter().map(|h| h.to_string()).collect::<Vec<_>>().join(" "));
    out += &format!("north_share {}\n", num("volume", "north_share")?);
    out += &format!("through_share {}\n", num("volume", "through_share")?);
    out += &format!("terminating {}\n", pair("terminating")?);
    let mut sum = 0.0;
    for (class, dwell) in CLASSES {
        let share = num("mix", class)?;
        sum += share;
        out += &format!("class {class} {share} {}\n", pair(dwell)?);
    }
    if (sum - 1.0).abs() > 1e-9 {
        return Err(format!("ndls.traffic.toml: the class shares sum to {sum}, not 1"));
    }
    let loco = num("length_m", "loco")?;
    let trains = doc.get("train").and_then(|t| t.as_array()).ok_or("ndls.traffic.toml: no [[train]] pool")?;
    for (i, t) in trains.iter().enumerate() {
        let s = |k: &str| t.get(k).and_then(|x| x.as_str()).ok_or(format!("ndls.traffic.toml: train #{i} has no {k}"));
        let (number, ty, name, stops) = (s("number")?, s("type")?, s("name")?, s("stops")?);
        let &(_, class, len_key, hauled) = TYPES.iter().find(|r| r.0 == ty).ok_or(format!("ndls.traffic.toml: train {number}: type '{ty}' has no class"))?;
        if !["starts", "terminates", "through"].contains(&stops) || number.contains(char::is_whitespace) || name.contains('\n') {
            return Err(format!("ndls.traffic.toml: train {number}: bad number, stops or name"));
        }
        let len = num("length_m", len_key)? + if hauled { loco } else { 0.0 };
        out += &format!("train {number} {class} {len:.1} {stops} {name}\n");
    }
    Ok(out)
}
