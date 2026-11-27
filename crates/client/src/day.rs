//! The world's day: where the sun is and what hour it is at a place, both a
//! function of the world's clock and the day's length alone.
//!
//! The sun turns about `+Y`, the axis north is read by, so a longitude is a
//! time of day. The planet keeps 24 time zones of 15° of longitude, time
//! zone 0 centred on the prime meridian, as a real clock's are; a place
//! reads the hour at its time zone's middle meridian, one hour for everyone
//! in it. On a world shaped by the Earth, longitudes are the Earth's
//! ([`topology::at_lat_lon`]), so a place there reads its solar time zone's
//! hour.

use core::f64::consts::TAU;

use glam::DVec3;

/// The day of a world whose admin never chose one, and of the offline
/// preview: minutes the sun takes round. A dropped link keeps the world's.
pub const DEFAULT_MINUTES: u32 = 24;

/// How far round its day the sun is when the world's clock reads zero,
/// radians: it stands 0.7 rad east of the prime meridian, so the prime
/// meridian reads mid morning, 09:19. A constant of the frame, never of a
/// place, so every world and every client of one agree on the sun.
const AT_ZERO: f64 = -0.7;

/// How far the sun goes between the directions its shadows are drawn from,
/// radians: a step every second of a 24 minute day, every ten of a 240
/// minute one. Between two, a shadow ten metres long moves a few
/// centimetres, under the width the maps soften an edge across. The step is
/// on the angle, so a shot's strips hold still at every length.
pub const SHADOW_STEP: f64 = 0.25 * core::f64::consts::PI / 180.0;

/// Degrees of longitude a time zone spans.
const ZONE_DEG: f64 = 15.0;

/// Time zones round the planet.
const ZONES: f64 = 360.0 / ZONE_DEG;

/// Degrees of latitude from either pole within which the planet reads one
/// time zone, the prime meridian's: the time zones meet there, and a body
/// would turn through all of them on the spot. Its edge is a line like any
/// time zone's, so the hour changes once across it and never shimmers.
pub const POLAR_CAP_DEG: f64 = 1.0;

/// Minutes in a day of the clock: an hour reads `0..1440`.
const MINUTES_OF_A_DAY: u32 = 24 * 60;

/// Minutes of the clock a time zone is ahead of the one west of it.
const ZONE_MIN: u32 = MINUTES_OF_A_DAY / ZONES as u32;

/// How far round its day the sun is at `clock_s` of a day `minutes` long,
/// radians. Nothing else enters: a day changed is a sun that jumps to where
/// the clock over the new length puts it, in every eye at once.
pub fn sun_angle(clock_s: f64, minutes: u32) -> f64 {
    AT_ZERO + clock_s / (f64::from(minutes) * 60.0) * TAU
}

/// Toward the sun `angle` radians round its day, from anywhere: it stands
/// over the longitude `-angle`, so it moves west as the angle grows.
pub fn sun_toward(angle: f64) -> DVec3 {
    DVec3::new(angle.cos(), 0.35, angle.sin()).normalize()
}

/// The sun the shadows are drawn from at `angle`: the step of its path it
/// has begun, the next, and how far between. A function of the clock and
/// the day alone, so every client and every shot of one clock draw the same
/// maps.
pub fn sun_steps(angle: f64) -> scene::SunSteps {
    let steps = angle / SHADOW_STEP;
    let begun = steps.floor();
    scene::SunSteps {
        from: sun_toward(begun * SHADOW_STEP).as_vec3(),
        to: sun_toward((begun + 1.0) * SHADOW_STEP).as_vec3(),
        blend: (steps - begun) as f32,
    }
}

/// The time zone a place on the planet is in, `0..24` eastward from the
/// prime meridian's, by its local `up`: the polar caps are time zone 0.
pub fn time_zone(up: [f64; 3]) -> u32 {
    if topology::latitude_deg(up).abs() >= 90.0 - POLAR_CAP_DEG {
        return 0;
    }
    topology::longitude_deg(up).map_or(0.0, |lon| (lon / ZONE_DEG).round().rem_euclid(ZONES)) as u32
}

/// The hour at a place on the planet when the sun is `angle` round its day,
/// in minutes past midnight, `0..1440`: the sun's hour at the middle
/// meridian of the place's time zone, noon when it stands highest there.
pub fn hour_min(up: [f64; 3], angle: f64) -> u32 {
    let day = f64::from(MINUTES_OF_A_DAY);
    // The prime meridian is past noon by as far as it stands east of the
    // sun, which stands over the longitude `-angle`.
    let prime = (day / 2.0 + angle / TAU * day).rem_euclid(day);
    let prime = (prime.floor() as u32).min(MINUTES_OF_A_DAY - 1);
    // Each time zone east is a whole hour later, added whole: a step across
    // a time zone's edge is an hour of the clock to the minute.
    (prime + time_zone(up) * ZONE_MIN) % MINUTES_OF_A_DAY
}

#[cfg(test)]
mod tests {
    use super::*;
    use topology::at_lat_lon;

    /// The lengths an admin chooses among, in minutes.
    fn lengths() -> [u32; 4] {
        protocol::DAY_LENGTHS.map(|length| length as u32)
    }

    /// How high the sun stands over a place: the sine of its elevation.
    fn height(up: [f64; 3], angle: f64) -> f64 {
        DVec3::from_array(up).normalize().dot(sun_toward(angle))
    }

    /// `h:mm` of minutes past midnight, for the failures.
    fn clock(minute: u32) -> String {
        format!("{}:{:02}", minute / 60, minute % 60)
    }

    #[test]
    fn the_sun_goes_round_once_a_day_of_each_length() {
        let prime = at_lat_lon(0.0, 0.0);
        for minutes in lengths() {
            let day_s = f64::from(minutes) * 60.0;
            for (clock_s, turns) in [
                (0.0, 0.0),
                (day_s / 4.0, 0.25),
                (day_s, 1.0),
                (7.5 * day_s, 7.5),
            ] {
                let angle = sun_angle(clock_s, minutes);
                assert!(
                    (angle - (AT_ZERO + turns * TAU)).abs() < 1e-9,
                    "{minutes} min at {clock_s} s"
                );
            }
            // Mid morning at the prime meridian whenever the clock is a whole
            // number of days, and a twenty-fourth of the day an hour later.
            for days in [0.0, 1.0, 3.0, 1000.0] {
                let at = |s: f64| clock(hour_min(prime, sun_angle(days * day_s + s, minutes)));
                assert_eq!(at(0.0), "9:19", "{minutes} min, day {days}");
                assert_eq!(at(day_s / 24.0), "10:19", "{minutes} min, day {days}");
                assert_eq!(at(day_s / 2.0), "21:19", "{minutes} min, day {days}");
            }
        }
    }

    /// Noon is when the sun stands highest at a time zone's middle meridian,
    /// and within half an hour of it everywhere in the time zone.
    #[test]
    fn noon_is_when_the_sun_is_highest_in_each_time_zone() {
        const SAMPLES: u32 = 2880;
        for minutes in lengths() {
            let day_s = f64::from(minutes) * 60.0;
            for zone_at in 0..24 {
                let middle = f64::from(zone_at) * ZONE_DEG;
                for (lat, off) in [
                    (0.0, 0.0),
                    (45.0, 0.0),
                    (-60.0, 0.0),
                    (20.0, -7.4),
                    (-35.0, 7.4),
                ] {
                    let up = at_lat_lon(lat, middle + off);
                    let highest = (0..SAMPLES)
                        .map(|i| day_s * 9.0 + day_s * f64::from(i) / f64::from(SAMPLES))
                        .max_by(|a, b| {
                            let (a, b) = (sun_angle(*a, minutes), sun_angle(*b, minutes));
                            height(up, a).total_cmp(&height(up, b))
                        })
                        .unwrap();
                    let read = hour_min(up, sun_angle(highest, minutes));
                    let from_noon = i64::from(read) - 720;
                    // Half a minute of sampling either way, and half an hour
                    // for a place off its time zone's middle.
                    let within = if off == 0.0 { 1 } else { 30 };
                    assert!(
                        from_noon.abs() <= within,
                        "time zone {zone_at}, {lat}° {}°, {minutes} min: highest at {}",
                        middle + off,
                        clock(read)
                    );
                }
            }
        }
    }

    #[test]
    fn a_time_zone_reads_one_minute_and_the_next_an_hour_later() {
        for minutes in lengths() {
            for clock_s in [0.0, 123.4, 5_000.0, 1.8e9] {
                let angle = sun_angle(clock_s, minutes);
                for zone_at in 0..24 {
                    let middle = f64::from(zone_at) * ZONE_DEG;
                    let here = hour_min(at_lat_lon(10.0, middle - 7.0), angle);
                    let there = hour_min(at_lat_lon(-50.0, middle + 6.9), angle);
                    assert_eq!(here, there, "two places of time zone {zone_at}");

                    // A step across the eastern edge: one hour later, to the minute.
                    let edge = middle + ZONE_DEG / 2.0;
                    let west = hour_min(at_lat_lon(30.0, edge - 1e-6), angle);
                    let east = hour_min(at_lat_lon(30.0, edge + 1e-6), angle);
                    assert_eq!(
                        (west + 60) % 1440,
                        east,
                        "across {edge}°: {} then {}",
                        clock(west),
                        clock(east)
                    );
                }
            }
        }
    }

    /// The caps read one time zone, the prime meridian's, so a body turning
    /// at a pole reads one hour; off them a longitude is one like any.
    #[test]
    fn a_polar_cap_is_one_time_zone() {
        let angle = sun_angle(777.0, 24);
        let prime = hour_min(at_lat_lon(0.0, 0.0), angle);
        for lon in [-170.0, -60.0, 0.0, 45.0, 120.0] {
            for lat in [90.0 - POLAR_CAP_DEG, 89.5, 90.0, -89.9, -90.0] {
                assert_eq!(
                    hour_min(at_lat_lon(lat, lon), angle),
                    prime,
                    "{lat}° {lon}°"
                );
            }
        }
        assert_eq!(
            time_zone(at_lat_lon(88.9, 120.0)),
            8,
            "off the cap, the time zone of its longitude"
        );
    }
}
